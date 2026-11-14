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
use ed_geo::{CanopyPage, HeightPage, NodeId, PAGE_N, PAGE_SAMPLES};
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
/// Index buffer LODs: the same 65 x 65 vertices drawn at stride 1, 2, 4 and 8 (64^2 ... 8^2
/// cells; the skirt ring follows the stride). All three live in one index buffer, `lod_index_offset`.
pub const LODS: usize = 4;
pub const fn lod_cells(l: u32) -> u32 {
    CELLS >> l
}
pub const fn lod_grid_tris(l: u32) -> u32 {
    2 * lod_cells(l) * lod_cells(l)
}
pub const fn lod_tris(l: u32) -> u32 {
    lod_grid_tris(l) + 8 * lod_cells(l)
}
/// First index of LOD `l` in the shared index buffer.
pub const fn lod_index_offset(l: u32) -> u32 {
    let mut o = 0;
    let mut m = 0;
    while m < l {
        o += lod_tris(m) * 3;
        m += 1;
    }
    o
}
/// Triangles of LOD 0 (the full-resolution mesh).
pub const INDEX_COUNT0: u32 = TRIS * 3;
/// Indices of all LODs.
pub const INDEX_COUNT: u32 = lod_index_offset(LODS as u32);
/// The visibility word's primitive carries the node's LOD at this bit (`t_triangle`).
pub const PRIM_LOD_SHIFT: u32 = 24;
/// Words per drawn node (`TerrainDraw` in terrain_common.slang).
pub const DRAW_WORDS: usize = 56;
/// Period (m) of the per-node texture phase (`TerrainDraw::tex`): every material and macro
/// noise scale divides it, so the wrap is invisible.
pub const TEX_PERIOD_M: f64 = 4096.0;
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
/// Bits 8-9: the node's index LOD (stride 1 << lod).
pub const F_LOD_SHIFT: u32 = 8;
/// Bits 10-21: per edge (same order as the coarse mask), 3 bits of log2 of the neighbour edge's
/// vertex spacing in this node's grid units (>= own stride): vertices between its lattice
/// points slide onto the segment (`t_grid_vertex`).
pub const F_EDGE_SHIFT: u32 = 10;
/// No vertex of the node morphs (the node lies wholly inside its own morph band start).
pub const F_NOMORPH: u32 = 1 << 22;
/// The far forest rung is on for this draw (trees G3): forest comes from the canopy page
/// (`tex.z`, 0 = no page = no forest), not from the old noise rule.
pub const F_TREES: u32 = 1 << 23;
/// `EW_TREES_FAR=debug`: the resolve tints canopy pixels by cover, height and mix.
pub const F_TREES_DEBUG: u32 = 1 << 24;
/// The draw has a ground page (own or parent, grass G4): the grass carpet colour comes from it.
pub const F_GROUND: u32 = 1 << 25;

/// u32 words per ground pool slot: one packed `[herb, crop, shrub, wet]` per page sample
/// (herb in the low byte; `ed_geo::GroundPage`).
pub const GROUND_WORDS: usize = PAGE_SAMPLES;
/// Rows of the terrain carpet table (`ed_world::grass::carpet_rows`, 16 floats each).
pub const CARPET_MAX_ROWS: usize = 16;
pub const CARPET_ROW_FLOATS: usize = 16;
/// Corner climate quantisation (`TerrainDraw::clim`): aridity 0..`CLIM_ARID_MAX`, temperature
/// `CLIM_T0`..`CLIM_T0 + CLIM_T_RANGE` C, 16 bits each. Mirrored in terrain_common.slang.
pub const CLIM_ARID_MAX: f32 = 4.0;
pub const CLIM_T0: f32 = -60.0;
pub const CLIM_T_RANGE: f32 = 100.0;

/// One corner's climate word: aridity in the low 16 bits, temperature in the high.
pub fn clim_word(c: [f32; 2]) -> u32 {
    let q = |x: f32| (x.clamp(0.0, 1.0) * 65535.0).round() as u32;
    q(c[0] / CLIM_ARID_MAX) | q((c[1] - CLIM_T0) / CLIM_T_RANGE) << 16
}

/// u32 words per canopy pool slot: one packed `[cover, height, mix, under]` per page sample.
pub const CANOPY_WORDS: usize = PAGE_SAMPLES;
/// The reference tree of the vertex-lift ramp (m): the lift starts when a tree this tall is
/// [`LIFT_NONE_PX`] tall on screen and is full at [`LIFT_FULL_PX`] (trees §3, T3). G5 ties these
/// to the impostor hand-off. Mirrored in terrain_common.slang (`T_LIFT_*`).
pub const TREE_REF_M: f64 = 15.0;
pub const LIFT_NONE_PX: f32 = 12.0;
pub const LIFT_FULL_PX: f32 = 6.0;

/// The far forest rung (trees G3), `EW_TREES_FAR`: `0` the old noise forest, `1` canopy
/// colour and shading from the canopy page without the vertex lift, `debug` lift plus a
/// cover/height/mix tint, anything else (the default) lift plus shading.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreesFar {
    #[default]
    Off,
    Colour,
    Full,
    Debug,
}

impl TreesFar {
    pub fn from_env() -> TreesFar {
        match std::env::var("EW_TREES_FAR").as_deref() {
            Ok("0") | Ok("off") => TreesFar::Off,
            Ok("1") | Ok("colour") => TreesFar::Colour,
            Ok("debug") => TreesFar::Debug,
            _ => TreesFar::Full,
        }
    }
    /// Canopy pages are loaded and drawn.
    pub fn on(self) -> bool {
        self != TreesFar::Off
    }
    /// Terrain vertices are lifted by the canopy.
    pub fn lift(self) -> bool {
        matches!(self, TreesFar::Full | TreesFar::Debug)
    }
}

/// Crown albedo per species family (linear, sunlit leaf mass, trees §3 "colour is conserved"):
/// the one table the canopy rung, impostor bakes and meshes share. Row 0 broadleaf, row 1
/// conifer. Mirrored in resolve.slang (`T_CROWN_*`; a test checks they agree).
pub const CROWN_ALBEDO: [[f32; 3]; 2] = [[0.050, 0.110, 0.024], [0.026, 0.064, 0.020]];
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
    /// The far forest rung (trees G3).
    pub trees_far: TreesFar,
    /// Ground pages on the GPU (grass G4, `EW_GROUND_PAGES`, needs the residency to build them):
    /// the grass carpet colour from the land-cover shares at every distance.
    pub ground_pages: bool,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self { max_draws: 4096, pool_slots: 1024, skirts: true, clear_targets: false, select: SelectConfig::default(), trees_far: TreesFar::Off, ground_pages: false }
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
    /// x, y: the node centre's face angle coordinates times the equatorial radius, mod
    /// [`TEX_PERIOD_M`] (world-stable texture coordinates without large f32 values).
    pub tex: [f32; 4],
    /// x, y: own and parent ground page word offsets in the pool (0: none); z, w: unused.
    pub gnd: [u32; 4],
    /// Climate at the node's corners ([`clim_word`]; `NodeId::corner` order).
    pub clim: [u32; 4],
    /// The land-cover lookup pages (grass G4b): x, y: ground and canopy page word offsets of
    /// the node's [`LANDCOVER_LEVEL`] ancestor (or its own at or above that level; 0: none),
    /// z: this node's page-sample origin in that page (x low 16 bits, y high, 1/128 sample),
    /// w (bits): its page-sample scale there. One smooth (bilinear 10 m) land cover under
    /// every finer level, the same pages the blades read.
    pub anc: [u32; 4],
}

/// Level of the land-cover pages the terrain carpet and the blades share (ew `PAGE_LEVEL`):
/// its ~9.5 m samples are about the 10 m source cells, so bilinear reads are smooth.
pub const LANDCOVER_LEVEL: u8 = 12;

/// `anc.z`, `anc.w` of a node at `id` in the page of its [`LANDCOVER_LEVEL`] ancestor.
pub fn landcover_origin(id: NodeId) -> (u32, f32) {
    let Some(k) = id.level().checked_sub(LANDCOVER_LEVEL).filter(|k| *k > 0) else {
        return (0, 1.0);
    };
    let a = id.ancestor(LANDCOVER_LEVEL);
    let (rx, ry) = (id.x() - (a.x() << k), id.y() - (a.y() << k));
    // Origin = rel * 256 / 2^k samples, in 1/128 sample: rel << (15 - k).
    let q = |r: u32| if k <= 15 { r << (15 - k) } else { r >> (k - 15) };
    (q(rx) | q(ry) << 16, 1.0 / (1u32 << k) as f32)
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
            w[40 + c] = self.tex[c].to_bits();
        }
        w[44..48].copy_from_slice(&self.gnd);
        w[48..52].copy_from_slice(&self.clim);
        w[52..56].copy_from_slice(&self.anc);
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
        tex: [(tu.atan() * a).rem_euclid(TEX_PERIOD_M) as f32, (tv.atan() * a).rem_euclid(TEX_PERIOD_M) as f32, 0.0, 0.0],
        gnd: [0; 4],
        clim: [0; 4],
        anc: [0; 4],
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
    let fl = d.slots[2];
    let edge = if i == 0 {
        Some((0, j))
    } else if i == CELLS {
        Some((1, j))
    } else if j == 0 {
        Some((2, i))
    } else if j == CELLS {
        Some((3, i))
    } else {
        None
    };
    if let Some((edge, e)) = edge {
        let cb = fl & (1 << edge) != 0;
        let ee = 1u32 << ((fl >> (F_EDGE_SHIFT + 3 * edge)) & 7);
        if cb && ee <= 2 {
            return cpu_coarse(d, pg, edge, e);
        }
        if ee > 1 {
            let lo = e & !(ee - 1);
            if lo == e {
                return cpu_base(d, pg, i, j);
            }
            let (a, b) = (edge_at(edge, lo), edge_at(edge, lo + ee));
            return cpu_base(d, pg, a.0, a.1).lerp(cpu_base(d, pg, b.0, b.1), (e - lo) as f32 / ee as f32);
        }
    }
    let p = cpu_base(d, pg, i, j);
    if fl & F_NOMORPH != 0 {
        return p;
    }
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

/// `t_triangle`: vertex numbers of primitive `prim` (the index buffer's order; the LOD rides at
/// [`PRIM_LOD_SHIFT`]).
pub fn triangle(prim: u32) -> [u32; 3] {
    let lod = (prim >> PRIM_LOD_SHIFT) & 3;
    let prim = prim & ((1 << PRIM_LOD_SHIFT) - 1);
    let (st, c) = (1u32 << lod, lod_cells(lod));
    if prim < lod_grid_tris(lod) {
        let q = prim >> 1;
        let (i, j) = ((q % c) * st, (q / c) * st);
        let v00 = j * SIDE + i;
        let (v10, v01) = (v00 + st, v00 + st * SIDE);
        let v11 = v01 + st;
        return if prim & 1 == 0 { [v00, v10, v11] } else { [v00, v11, v01] };
    }
    let k = prim - lod_grid_tris(lod);
    let s = k >> 1;
    let s1 = (s + 1) % (4 * c);
    let (rp, rq) = (s * st, s1 * st);
    let (p, q) = (ring(rp), ring(rq));
    let (tp, tq) = (p.1 * SIDE + p.0, q.1 * SIDE + q.0);
    let (a, b) = (GRID_VERTS + rp, GRID_VERTS + rq);
    if k & 1 == 0 { [a, b, tq] } else { [a, tq, tp] }
}

/// Is `prim` (with its LOD bits) a skirt triangle?
pub fn prim_is_skirt(prim: u32) -> bool {
    prim & ((1 << PRIM_LOD_SHIFT) - 1) >= lod_grid_tris((prim >> PRIM_LOD_SHIFT) & 3)
}

/// Triangles one frame of the latest `prepare` drew: (view, the four cascades).
impl TerrainProvider {
    pub fn tri_counts(&self) -> (u64, [u64; 4]) {
        let v = &self.views[self.cur];
        let sum = |g: &[(u32, u32); LODS]| g.iter().enumerate().map(|(l, x)| x.1 as u64 * lod_tris(l as u32) as u64).sum::<u64>();
        (sum(&v.groups[0]), std::array::from_fn(|c| sum(&v.groups[c + 1])))
    }
    /// Nodes per index LOD in the view (the latest `prepare`).
    pub fn lod_counts(&self) -> [u32; LODS] {
        self.views[self.cur].groups[0].map(|g| g.1)
    }
}

/// The shared index buffer: LOD 0, then 1, then 2.
pub fn indices() -> Vec<u32> {
    (0..LODS as u32).flat_map(|l| (0..lod_tris(l)).flat_map(move |p| triangle(p | l << PRIM_LOD_SHIFT))).collect()
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
// Index LOD selection
// ---------------------------------------------------------------------------------------------

/// A view's index LOD for a node is the coarsest stride whose projected cells stay under both
/// limits: effective cell area (`s * sqrt(px^2 per fine cell)`) and the longest cell edge.
fn lod_limits() -> (f32, f32) {
    static L: std::sync::OnceLock<(f32, f32)> = std::sync::OnceLock::new();
    *L.get_or_init(|| {
        let f = |k: &str, d: f32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        (f("ED_TERRAIN_LOD_AREA", 12.0), f("ED_TERRAIN_LOD_EDGE", 48.0))
    })
}

/// Cell size (m) of a draw's grid.
fn cell_m(d: &TerrainDraw) -> f32 {
    d.shape[0] * d.dir_c[3]
}

/// Cascade LOD: the coarsest stride whose cells are at most `CASCADE_TEXELS` texels.
fn cascade_lod(d: &TerrainDraw, texel_m: f32, c: usize) -> u8 {
    let mut k = 0u8;
    while (k as usize) + 1 < LODS && cell_m(d) * (2 << k) as f32 <= cascade_texels(c) * texel_m {
        k += 1;
    }
    k
}
/// Texels a caster cell may span in cascade `c`: the first cascade (next to the character) stays
/// near the main view's detail so shadow and surface agree, outer cascades are far coarser.
fn cascade_texels(c: usize) -> f32 {
    static T: std::sync::OnceLock<[f32; 4]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let v: Vec<f32> = std::env::var("ED_TERRAIN_CASCADE_TEXELS").ok().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
        if v.len() == 4 { [v[0], v[1], v[2], v[3]] } else { [2.0, 4.0, 8.0, 8.0] }
    })[c.min(3)]
}

/// Meters per texel of an orthographic cascade matrix (camera-relative clip, 2 units wide).
fn cascade_texel_m(m: &glam::Mat4, tile: u32) -> f32 {
    let per_m = m.row(0).truncate().length().max(1e-9);
    2.0 / (per_m * tile as f32)
}

/// Clamps `want` (log2 strides) so a finer-level node never has a coarser edge spacing than its
/// coarser neighbour (the coarser node does not stitch to it), then writes the LOD and per-edge
/// stitching spacing flags into `entries`. Returns the final LODs.
fn assign_lods(entries: &mut [TerrainDraw], want: &[u8]) -> Vec<u8> {
    let idx: BTreeMap<NodeId, usize> = entries.iter().enumerate().filter_map(|(i, d)| d.id.map(|id| (id, i))).collect();
    // Per node and edge: (neighbour index, same level?).
    let links: Vec<[Option<(usize, bool)>; 4]> = entries
        .iter()
        .map(|d| {
            let mut out = [None; 4];
            if let Some(id) = d.id {
                for (k, (dx, dy)) in [(-1, 0), (1, 0), (0, -1), (0, 1)].into_iter().enumerate() {
                    let nb = id.neighbour(dx, dy);
                    if nb == id {
                        continue;
                    }
                    if let Some(&j) = idx.get(&nb) {
                        out[k] = Some((j, true));
                    } else if let Some(&j) = nb.parent().and_then(|p| idx.get(&p)) {
                        out[k] = Some((j, false));
                    }
                }
            }
            out
        })
        .collect();
    let mut l = want.to_vec();
    loop {
        let mut changed = false;
        for i in 0..l.len() {
            for (j, same) in links[i].iter().flatten() {
                if !same && l[i] > l[*j] + 1 {
                    l[i] = l[*j] + 1;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for (i, d) in entries.iter_mut().enumerate() {
        let mut fl = d.slots[2] & !(3 << F_LOD_SHIFT) & !(0xFFF << F_EDGE_SHIFT);
        fl |= (l[i] as u32) << F_LOD_SHIFT;
        for k in 0..4 {
            let e = match links[i][k] {
                Some((j, true)) => l[i].max(l[j]),
                Some((j, false)) => l[j] + 1,
                None => l[i],
            };
            // Coarse edges always carry their spacing (>= 2) so the stitch path is taken.
            fl |= (e as u32 & 7) << (F_EDGE_SHIFT + 3 * k as u32);
        }
        d.slots[2] = fl;
    }
    l
}

/// Stable sort of parallel arrays by LOD; returns the (first, count) of each LOD.
fn sort_by_lod<T>(lods: &[u8], items: &mut Vec<T>) -> [(u32, u32); LODS] {
    let mut order: Vec<usize> = (0..lods.len()).collect();
    order.sort_by_key(|&i| lods[i]);
    let mut slots: Vec<Option<T>> = items.drain(..).map(Some).collect();
    items.extend(order.iter().map(|&i| slots[i].take().unwrap()));
    let mut g = [(0u32, 0u32); LODS];
    let mut first = 0;
    for (k, gk) in g.iter_mut().enumerate() {
        let n = lods.iter().filter(|&&x| x as usize == k).count() as u32;
        *gk = (first, n);
        first += n;
    }
    g
}

// ---------------------------------------------------------------------------------------------
// Page pool
// ---------------------------------------------------------------------------------------------

struct PoolEntry {
    slot: u32,
    stamp: u64,
    page: Arc<HeightPage>,
    /// The canopy page uploaded into this slot's canopy words, and its tallest canopy (m).
    canopy: Option<(Arc<CanopyPage>, f32)>,
    /// The ground page uploaded into this slot's ground words (grass G4).
    ground: Option<Arc<ed_geo::GroundPage>>,
}

/// GPU height page pool bookkeeping: `NodeId -> slot`, least recently used first (ties by id).
struct Pool {
    map: BTreeMap<NodeId, PoolEntry>,
    free: Vec<u32>,
    /// Uploads: byte offset into the pool buffer, bytes.
    pending: Vec<(u64, Vec<u8>)>,
    /// Slots (the canopy words start after the height words of every slot).
    slots: u32,
    /// Canopy words per slot (0: no canopy pages); the ground words follow the canopy words.
    canopy_words: u32,
}

impl Pool {
    fn new(slots: u32, canopy: bool) -> Self {
        Self { map: BTreeMap::new(), free: (0..slots).rev().collect(), pending: Vec::new(), slots, canopy_words: if canopy { CANOPY_WORDS as u32 } else { 0 } }
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
            self.pending.push((slot as u64 * PAGE_WORDS as u64 * 4, page_bytes(page)));
            return Some(slot);
        }
        let slot = match self.free.pop() {
            Some(s) => s,
            None => {
                let (&victim, _) = self.map.iter().filter(|(_, e)| e.stamp < stamp).min_by_key(|(id, e)| (e.stamp, **id))?;
                self.map.remove(&victim).expect("victim").slot
            }
        };
        self.map.insert(id, PoolEntry { slot, stamp, page: page.clone(), canopy: None, ground: None });
        self.pending.push((slot as u64 * PAGE_WORDS as u64 * 4, page_bytes(page)));
        Some(slot)
    }

    /// First u32 word of `slot`'s canopy page in the pool buffer.
    fn canopy_word(&self, slot: u32) -> u32 {
        (self.slots as usize * PAGE_WORDS + slot as usize * CANOPY_WORDS) as u32
    }

    /// First u32 word of `slot`'s ground page in the pool buffer.
    fn ground_word(&self, slot: u32) -> u32 {
        (self.slots as usize * (PAGE_WORDS + self.canopy_words as usize) + slot as usize * GROUND_WORDS) as u32
    }

    /// Ground word offset of `id` (whose height page is already in the pool), uploading `page`
    /// into the slot's ground words if it is not there yet. 0: no page.
    fn ensure_ground(&mut self, id: NodeId, page: Option<&Arc<ed_geo::GroundPage>>) -> u32 {
        let Some(page) = page else { return 0 };
        let Some(e) = self.map.get_mut(&id) else { return 0 };
        let slot = e.slot;
        if e.ground.as_ref().is_some_and(|g| Arc::ptr_eq(g, page)) {
            return self.ground_word(slot);
        }
        e.ground = Some(page.clone());
        let off = self.ground_word(slot);
        self.pending.push((off as u64 * 4, page.to_bytes()));
        off
    }

    /// Canopy word offset and tallest canopy (m) of `id` (whose height page is already in the
    /// pool), uploading `page` into the slot's canopy words if it is not there yet. `None`
    /// page: no canopy (offset 0).
    fn ensure_canopy(&mut self, id: NodeId, page: Option<Arc<CanopyPage>>) -> (u32, f32) {
        let Some(page) = page else { return (0, 0.0) };
        let Some(e) = self.map.get_mut(&id) else { return (0, 0.0) };
        let slot = e.slot;
        if let Some((c, m)) = &e.canopy
            && Arc::ptr_eq(c, &page)
        {
            let m = *m;
            return (self.canopy_word(slot), m);
        }
        let max_m = ed_geo::decode_height(page.samples.iter().filter(|s| s[0] > 0).map(|s| s[1]).max().unwrap_or(0));
        e.canopy = Some((page.clone(), max_m));
        let off = self.canopy_word(slot);
        self.pending.push((off as u64 * 4, page.to_bytes()));
        (off, max_m)
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

type Consts = [u8; ed_gpu::DRAW_CONSTANTS_BYTES];

/// Material layer slots of the terrain material set (P-slice S7 c), in texture-array order.
/// The resolve blends them by slope, altitude, aspect, curvature and the height maps.
pub const MAT_LAYERS: [&str; 6] = ["rock", "scree", "grass", "forest", "snow", "soil"];

/// One surface layer: square RGBA8 mip chains (mip 0 first, each `size >> m` texels square).
#[derive(Clone, Debug, Default)]
pub struct TerrainLayer {
    /// Physical size of one texture repeat (m).
    pub tile_m: f32,
    /// Linear albedo calibration multiplied into the texture (target / texture mean).
    pub gain: [f32; 3],
    /// rgb: base colour (sRGB), a: displacement (0..1).
    pub albedo: Vec<Vec<u8>>,
    /// r, g: tangent-space normal x, y (OpenGL, +y up), b: roughness, a: ambient occlusion.
    pub normal: Vec<Vec<u8>>,
}

/// The terrain's surface materials, one [`TerrainLayer`] per [`MAT_LAYERS`] slot.
#[derive(Clone, Debug, Default)]
pub struct TerrainMaterials {
    pub size: u32,
    pub layers: Vec<TerrainLayer>,
}

/// Words of the material record the resolve reads (`TerrainMat` in terrain_common.slang).
pub const MAT_WORDS: usize = DRAW_WORDS;

/// Grass shells (styles C and D): most nodes drawn with shells.
pub const SHELL_MAX_DRAWS: usize = 64;
/// Grass type rows the shells and the tint read (`GrassType`, 32 floats each).
pub const SHELL_MAX_TYPES: usize = 64;
pub const SHELL_TYPE_FLOATS: usize = 32;
/// Cells per side of each shell map (ed_world::grass::MAP_N).
pub const SHELL_MAP_N: usize = 256;
/// Most shells (the visibility word carries the shell in 4 bits).
pub const SHELL_MAX: u32 = 16;

/// Where the shell records live in a view's draw table, after the material record: the shell
/// draws (record index), the type rows (`GrassType` index) and the fine map's first word (the
/// coarse map follows), and the table's total words.
pub fn shell_layout(max_draws: u32) -> (u32, u32, u32, usize) {
    let draw_base = max_draws * 5 + 1;
    let types_word = ((draw_base as usize + SHELL_MAX_DRAWS) * DRAW_WORDS).next_multiple_of(SHELL_TYPE_FLOATS);
    let map_word = types_word + SHELL_MAX_TYPES * SHELL_TYPE_FLOATS;
    (draw_base, (types_word / SHELL_TYPE_FLOATS) as u32, map_word as u32, map_word + 2 * SHELL_MAP_N * SHELL_MAP_N)
}

/// First word of the carpet table in a view's draw table: after the shell records.
pub fn carpet_word(max_draws: u32) -> usize {
    shell_layout(max_draws).3
}

/// Grass shells and the terrain's grass tint ([`TerrainProvider::set_grass`]): the type rows
/// and the two type maps of `ed_world::grass` (fine for the shells, coarse for the tint).
#[derive(Clone, Debug)]
pub struct ShellGrass {
    /// `ed_world::grass::gpu_rows`.
    pub types: Vec<[f32; SHELL_TYPE_FLOATS]>,
    /// The fine map, then the coarse one (`SHELL_MAP_N`^2 words each); `None`: not built yet.
    pub maps: Option<Arc<Vec<u32>>>,
    /// The maps' cube face.
    pub face: u8,
    /// Map centre in face metres modulo [`TEX_PERIOD_M`] (a multiple of the 32 m snap).
    pub centre: [f32; 2],
    /// Toroidal slot offsets: fine x, y, coarse x, y (`ed_world::grass::map_offset`).
    pub offsets: [u32; 4],
    /// Fine and coarse cell (m).
    pub cells: [f32; 2],
    /// Shells near the camera (0: no shells; at most [`SHELL_MAX`]).
    pub shells: u32,
    /// Shell range (m).
    pub range: f32,
    /// Terrain tint strength towards the grass carpet colour (0: off).
    pub tint: f32,
    /// Top shell height (m).
    pub h_max: f32,
}

struct MatTex {
    albedo: (ed_gpu::TextureId, ed_gpu::BindlessTexture),
    normal: (ed_gpu::TextureId, ed_gpu::BindlessTexture),
    size: u32,
    state: ResourceState,
    tile_m: [f32; 8],
    gain: [[f32; 3]; 6],
}

struct TerrainView {
    /// Main draws, then each cascade's culled copies (`ranges`).
    draws: Tracked,
    entries: Vec<TerrainDraw>,
    /// The main pass's draws (the entries inside the frustum, grouped by index LOD): the
    /// buffer's first `vis.len()` draws (set by `prepare`).
    vis: Vec<TerrainDraw>,
    /// Camera-relative box per entry (min xyz, max xyz).
    bounds: Vec<[f32; 6]>,
    /// Per cascade: first draw in the buffer and count (set by `prepare`).
    ranges: [(u32, u32); 4],
    /// Per list (0: the view, 1-4: cascades), per index LOD: first draw and count.
    groups: [[(u32, u32); LODS]; 5],
    /// Shell draws (index LOD per draw; draw `shell_layout().0 + i`).
    shells: Vec<u8>,
    imports: Vec<(Res, u8)>,
}

/// Provider for ed_planet terrain (Phase 2 W3).
pub struct TerrainProvider {
    pub config: TerrainConfig,
    built: Vec<ShaderId>,
    raster: RenderPipelineId,
    shadow: RenderPipelineId,
    shell: RenderPipelineId,
    /// Grass shells and tint ([`TerrainProvider::set_grass`]).
    grass: Option<ShellGrass>,
    index: Tracked,
    pool_buf: Tracked,
    pool: Pool,
    views: Vec<TerrainView>,
    cur: usize,
    stamp: u64,
    stats: TerrainStats,
    /// The eye of the latest update (draw origins are relative to it).
    eye: DVec3,
    /// Texels per cascade side (from the latest `record`; sizes the cascades' LODs).
    shadow_tile: u32,
    /// Surface material arrays ([`TerrainProvider::set_materials`]); `None`: the flat tint.
    mats: Option<MatTex>,
    /// The terrain carpet table (grass G4, [`TerrainProvider::set_carpet`]).
    carpet: Option<Vec<[f32; CARPET_ROW_FLOATS]>>,
}

const GEN_MSL: &str = include_str!("../shaders/gen/msl/terrain.metal");
const GEN_VS: &[u8] = include_bytes!("../shaders/gen/spv/terrain.vs_terrain.spv");
const GEN_FS: &[u8] = include_bytes!("../shaders/gen/spv/terrain.fs_terrain.spv");
const GEN_VS_SHADOW: &[u8] = include_bytes!("../shaders/gen/spv/terrain.vs_terrain_shadow.spv");
const GEN_VS_TREES: &[u8] = include_bytes!("../shaders/gen/spv/terrain.vs_terrain_trees.spv");
const GEN_VS_SHADOW_TREES: &[u8] = include_bytes!("../shaders/gen/spv/terrain.vs_terrain_shadow_trees.spv");
const GEN_VS_SHELL: &[u8] = include_bytes!("../shaders/gen/spv/terrain.vs_shell.spv");
const GEN_FS_SHELL: &[u8] = include_bytes!("../shaders/gen/spv/terrain.fs_shell.spv");

fn spv_words(b: &[u8]) -> Vec<u32> {
    b.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect()
}

impl TerrainProvider {
    pub fn new(dev: &dyn Device, config: TerrainConfig) -> Result<Self, ed_gpu::Error> {
        let mut built = Vec::new();
        let r = Self::pipelines(dev, &mut built, config.trees_far.lift());
        let (raster, shadow, shell) = match r {
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
        // Height words of every slot, then (far forest rung on) the canopy words of every slot.
        // Then (grass G4) the ground words of every slot.
        let words = PAGE_WORDS as u64 + if config.trees_far.on() { CANOPY_WORDS as u64 } else { 0 } + if config.ground_pages { GROUND_WORDS as u64 } else { 0 };
        let pool_buf = Tracked::new(dev, "terrain page pool", config.pool_slots as u64 * words * 4, upload)?;
        let view0 = Self::new_view(dev, config.max_draws)?;
        Ok(Self { config, built, raster, shadow, shell, grass: None, index, pool_buf, pool: Pool::new(config.pool_slots, config.trees_far.on()), views: vec![view0], cur: 0, stamp: 0, stats: TerrainStats::default(), eye: DVec3::ZERO, shadow_tile: 2048, mats: None, carpet: None })
    }

    fn new_view(dev: &dyn Device, max_draws: u32) -> Result<TerrainView, ed_gpu::Error> {
        let per_frame = Usage { shader_read: true, cpu_write: true, ..Default::default() };
        // Room for the view's draws and up to four cascade lists.
        // One more record at the end: the material record (`material_record`).
        // Then the grass shell records (`shell_layout`).
        // Then the carpet table (`carpet_word`).
        let size = (carpet_word(max_draws) + CARPET_MAX_ROWS * CARPET_ROW_FLOATS) as u64 * 4;
        Ok(TerrainView { draws: Tracked::new(dev, "terrain draws", size, per_frame)?, entries: Vec::new(), vis: Vec::new(), bounds: Vec::new(), ranges: [(0, 0); 4], groups: [[(0, 0); LODS]; 5], shells: Vec::new(), imports: Vec::new() })
    }

    /// `lift`: the vertex entries with the canopy lift (trees G3), else the exact unlifted ones.
    fn pipelines(dev: &dyn Device, built: &mut Vec<ShaderId>, lift: bool) -> Result<(RenderPipelineId, RenderPipelineId, RenderPipelineId), ed_gpu::Error> {
        let vk = dev.caps().backend == Backend::Vulkan;
        let (vs_e, vss_e) = if lift { ("vs_terrain_trees", "vs_terrain_shadow_trees") } else { ("vs_terrain", "vs_terrain_shadow") };
        let (vs_b, vss_b) = if lift { (GEN_VS_TREES, GEN_VS_SHADOW_TREES) } else { (GEN_VS, GEN_VS_SHADOW) };
        let (vs_w, fs_w, vss_w) = (spv_words(vs_b), spv_words(GEN_FS), spv_words(vss_b));
        let mut shader = |entry: &str, stage, spv: &[u32]| -> Result<ShaderId, ed_gpu::Error> {
            let code = if vk { ShaderCode::SpirV(spv) } else { ShaderCode::MslSource(GEN_MSL) };
            let s = dev.create_shader(&ShaderDesc { label: entry, stage, entry, code })?;
            built.push(s);
            Ok(s)
        };
        let vs = shader(vs_e, ShaderStage::Vertex, &vs_w)?;
        let fs = shader("fs_terrain", ShaderStage::Fragment, &fs_w)?;
        let vss = shader(vss_e, ShaderStage::Vertex, &vss_w)?;
        let vsh = shader("vs_shell", ShaderStage::Vertex, &spv_words(GEN_VS_SHELL))?;
        let fsh = shader("fs_shell", ShaderStage::Fragment, &spv_words(GEN_FS_SHELL))?;
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
        let shadow = match shadow {
            Ok(s) => s,
            Err(e) => {
                dev.destroy_render_pipeline(raster);
                return Err(e);
            }
        };
        // Grass shells: strictly in front of the ground (a shell flattened onto it, where no
        // grass grows, never wins), no back faces (shells face up like the ground).
        let shell = dev.create_render_pipeline(&RenderPipelineDesc {
            label: "terrain shells",
            vertex: vsh,
            fragment: Some(fsh),
            color_formats: &[VIS_FORMAT],
            blend: ed_gpu::Blend::Opaque,
            depth: Some(DepthDesc { format: DEPTH_FORMAT, write: true, compare: Compare::Greater, bias: 0.0, slope_bias: 0.0, clamp: false }),
            cull: Cull::Back,
            topology: Topology::Triangles,
            samples: 1,
        });
        match shell {
            Ok(sh) => Ok((raster, shadow, sh)),
            Err(e) => {
                dev.destroy_render_pipeline(raster);
                dev.destroy_render_pipeline(shadow);
                Err(e)
            }
        }
    }

    /// The terrain carpet table (grass G4, `ed_world::grass::carpet_rows`): with ground pages
    /// on, the grass and bare ground colour from the land-cover shares. `None`: no table (the
    /// draws keep the old tint).
    pub fn set_carpet(&mut self, rows: Option<Vec<[f32; CARPET_ROW_FLOATS]>>) {
        self.carpet = rows.map(|mut r| {
            r.truncate(CARPET_MAX_ROWS);
            r
        });
    }

    /// Grass shells and the terrain's grass tint (styles C and D); `None` turns both off.
    pub fn set_grass(&mut self, g: Option<ShellGrass>) {
        self.grass = g;
    }
    pub fn grass(&self) -> Option<&ShellGrass> {
        self.grass.as_ref()
    }

    /// Nearest distance (m) from the camera to a drawn node: its centre at its own page height,
    /// less the node's half diagonal.
    fn draw_near_m(&self, d: &TerrainDraw) -> f32 {
        let h = self.draw_pages(d).map_or(0.0, |pg| own_h(d, &pg, CELLS / 2, CELLS / 2));
        let c = v3(d.origin) + cpu_lattice(d, CELLS / 2, CELLS / 2, h);
        (c.length() - 0.75 * CELLS as f32 * cell_m(d)).max(0.0)
    }

    pub fn stats(&self) -> TerrainStats {
        self.stats
    }

    /// Uploads the surface material arrays (replacing any earlier set). `ED_TERRAIN_NOMAT=1`
    /// at run time keeps the flat slope/altitude tint instead (the off switch).
    pub fn set_materials(&mut self, dev: &dyn Device, m: &TerrainMaterials) -> Result<(), ed_gpu::Error> {
        self.clear_materials(dev);
        let n = m.layers.len().min(8) as u32;
        let mips = m.layers.first().map_or(0, |l| l.albedo.len().min(l.normal.len())) as u32;
        if n == 0 || mips == 0 || m.size == 0 {
            return Ok(());
        }
        let make = |label: &str, format: ed_gpu::Format, pick: &dyn Fn(&TerrainLayer) -> &Vec<Vec<u8>>| -> Result<(ed_gpu::TextureId, ed_gpu::BindlessTexture), ed_gpu::Error> {
            let id = dev.create_texture(&ed_gpu::TextureDesc {
                label,
                kind: ed_gpu::TextureKind::D2Array,
                format,
                width: m.size,
                height: m.size,
                depth_or_layers: n,
                mips,
                samples: 1,
                usage: Usage { shader_read: true, copy_dst: true, ..Default::default() },
            })?;
            for (l, layer) in m.layers.iter().take(n as usize).enumerate() {
                for (mip, data) in pick(layer).iter().take(mips as usize).enumerate() {
                    if let Err(e) = dev.write_texture(id, ed_gpu::Subresource { mip: mip as u32, layer: l as u32 }, data) {
                        dev.destroy_texture(id);
                        return Err(e);
                    }
                }
            }
            match dev.bind_texture(id) {
                Ok(b) => Ok((id, b)),
                Err(e) => {
                    dev.destroy_texture(id);
                    Err(e)
                }
            }
        };
        let albedo = make("terrain albedo", ed_gpu::Format::Rgba8Srgb, &|l| &l.albedo)?;
        let normal = match make("terrain normal", ed_gpu::Format::Rgba8Unorm, &|l| &l.normal) {
            Ok(t) => t,
            Err(e) => {
                dev.unbind_texture(albedo.1);
                dev.destroy_texture(albedo.0);
                return Err(e);
            }
        };
        let mut tile_m = [1.0f32; 8];
        let mut gain = [[1.0f32; 3]; 6];
        for (k, l) in m.layers.iter().enumerate() {
            if k < 8 {
                tile_m[k] = l.tile_m.max(0.05);
            }
            if k < 6 {
                gain[k] = l.gain;
            }
        }
        self.mats = Some(MatTex { albedo, normal, size: m.size, state: ResourceState::CopyDst, tile_m, gain });
        Ok(())
    }

    fn clear_materials(&mut self, dev: &dyn Device) {
        if let Some(t) = self.mats.take() {
            for (id, b) in [t.albedo, t.normal] {
                dev.unbind_texture(b);
                dev.destroy_texture(id);
            }
        }
    }

    /// Whether the material arrays are in use (set and not switched off).
    pub fn materials_on(&self) -> bool {
        self.mats.is_some() && std::env::var_os("ED_TERRAIN_NOMAT").is_none_or(|v| v == "0")
    }

    /// Record index (in the draw table) of the material record the resolve reads; 0: none
    /// (the flat tint). Record 0 is always a draw, so 0 never names the material record.
    pub fn material_record(&self) -> u32 {
        if self.materials_on() { self.config.max_draws * 5 } else { 0 }
    }

    /// The material record's words: texture indices, layer tile sizes, layer count.
    fn material_words(&self) -> [u32; MAT_WORDS] {
        let mut w = [0u32; MAT_WORDS];
        if let Some(t) = &self.mats {
            w[0] = t.albedo.1.0;
            w[1] = t.normal.1.0;
            w[2] = MAT_LAYERS.len() as u32;
            w[3] = t.size;
            for (k, x) in t.tile_m.iter().enumerate() {
                w[4 + k] = x.to_bits();
            }
            for (k, g) in t.gain.iter().enumerate() {
                for c in 0..3 {
                    w[12 + 4 * k + c] = g[c].to_bits();
                }
            }
        }
        w
    }

    /// Draws per sun cascade after culling (the latest `prepare` of this view).
    pub fn cascade_draws(&self) -> [u32; 4] {
        self.views[self.cur].ranges.map(|r| r.1)
    }

    /// This view's draws of the latest update, in draw (instance) order.
    pub fn draws(&self) -> &[TerrainDraw] {
        &self.views[self.cur].vis
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
            // Canopy pages ride in the same slots (trees G3); the height slots are taken first,
            // so a canopy upload never evicts a height page.
            let tf = self.config.trees_far;
            let (canopy, parent_canopy, lift_max) = if tf.on() {
                let (own, max_m) = self.pool.ensure_canopy(id, res.canopy_page(id));
                let par = match (&parent, id.parent()) {
                    (Some(_), Some(p)) => self.pool.ensure_canopy(p, res.canopy_page(p)).0,
                    _ => 0,
                };
                (own, par, if tf.lift() { max_m } else { 0.0 })
            } else {
                (0, 0, 0.0)
            };
            // Ground pages (grass G4) ride in the same slots, after the canopy words.
            // The land-cover pages (grass G4b): the LANDCOVER_LEVEL ancestor's (own at or above
            // it). Its height page takes a slot first, so the ground and canopy can ride in it.
            let landcover = if self.config.ground_pages && self.carpet.is_some() {
                let a = id.ancestor(LANDCOVER_LEVEL);
                let slot = if a == id { Some(slot) } else { res.height_page(a).and_then(|pg| self.pool.ensure(a, &pg, stamp)) };
                match (slot, res.ground(a)) {
                    (Some(_), Some(g)) => {
                        let go = self.pool.ensure_ground(a, Some(&g.page));
                        let co = if tf.on() { self.pool.ensure_canopy(a, res.canopy_page(a)).0 } else { 0 };
                        let (o, sc) = landcover_origin(id);
                        [go, co, o, sc.to_bits()]
                    }
                    _ => [0; 4],
                }
            } else {
                [0; 4]
            };
            let ground = if self.config.ground_pages && self.carpet.is_some() {
                let own = res.ground(id);
                let par = parent.as_ref().and(id.parent()).and_then(|p| res.ground(p).map(|g| (p, g)));
                let o = self.pool.ensure_ground(id, own.as_ref().map(|g| &g.page));
                let po = par.as_ref().map_or(0, |(p, g)| self.pool.ensure_ground(*p, Some(&g.page)));
                // Corner climate: the own page's, else the parent's (bilinear at the corners
                // would be exact on a coarser grid; the parent's corners are close enough for a
                // draw that is waiting on its own page).
                let clim = own.as_ref().map(|g| g.climate).or_else(|| par.as_ref().map(|(_, g)| g.climate));
                (o, po, clim)
            } else {
                (0, 0, None)
            };
            let coarse = coarse_mask(id, &drawn);
            if coarse != 0 {
                st.stitched += 1;
            }
            let cfg = &self.config.select;
            let bands = [morph_band(body, id.level(), cfg, pixels_per_radian), morph_band(body, id.level().saturating_sub(1), cfg, pixels_per_radian)];
            let mut d = node_draw(id, body, pose, eye, bands, self.config.skirts, (slot, &page), parent.as_ref().map(|(s, p)| (*s, &**p)), coarse);
            if let (o, po, Some(clim)) = ground
                && (o != 0 || po != 0)
            {
                d.slots[2] |= F_GROUND;
                d.gnd = [o, po, 0, 0];
                d.clim = clim.map(clim_word);
                d.anc = landcover;
            }
            if tf.on() {
                d.slots[2] |= F_TREES | if tf == TreesFar::Debug { F_TREES_DEBUG } else { 0 };
                d.tex[2] = f32::from_bits(canopy);
                d.tex[3] = f32::from_bits(parent_canopy);
                if tf.lift() {
                    // Metres times pixels per radian: a tree of height h at distance r is
                    // `h * polar.w / (TREE_REF_M * r)` pixels tall.
                    d.polar[3] = (TREE_REF_M * pixels_per_radian) as f32;
                }
            }
            bounds.push(node_bound(body, pose, eye, id, &page, d.origin[3] as f64 + lift_max as f64));
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

    /// Index LOD of a node in the main view: 5 x 5 lattice points are projected with `vp` (the
    /// camera-relative view-projection); the densest 16 x 16-cell patch sets the stride. A node
    /// crossing the camera plane or without a page stays at LOD 0.
    fn main_lod(&self, d: &TerrainDraw, vp: &glam::Mat4, w: f32, h: f32) -> u8 {
        let Some(pg) = self.draw_pages(d) else { return 0 };
        let mut px = [[glam::Vec2::ZERO; 5]; 5];
        let mut front = [[false; 5]; 5];
        for (j, row) in px.iter_mut().enumerate() {
            for (i, p) in row.iter_mut().enumerate() {
                let (gi, gj) = (i as u32 * 16, j as u32 * 16);
                let q = v3(d.origin) + cpu_lattice(d, gi, gj, own_h(d, &pg, gi, gj));
                let c = *vp * q.extend(1.0);
                front[j][i] = c.w > 1e-2;
                if front[j][i] {
                    *p = glam::Vec2::new(c.x / c.w, c.y / c.w);
                }
            }
        }
        let cross = |u: glam::Vec2, v: glam::Vec2| u.x * v.y - u.y * v.x;
        let scale = glam::Vec2::new(0.5 * w, 0.5 * h);
        let (mut dens, mut edge) = (0.0f32, 0.0f32);
        for j in 0..4 {
            for i in 0..4 {
                let ij = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
                let on = ij.iter().filter(|&&(a, b)| front[b][a]).count();
                if on == 0 {
                    continue;
                }
                let pts = ij.map(|(a, b)| px[b][a]);
                let (lo, hi) = pts.iter().fold((glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN)), |(l, h), p| (l.min(*p), h.max(*p)));
                let seen = ij.iter().zip(&pts).any(|(&(a, b), p)| front[b][a] && p.abs().max_element() <= 1.5) || (hi.x >= -1.0 && lo.x <= 1.0 && hi.y >= -1.0 && lo.y <= 1.0);
                if on < 4 {
                    // Straddles the camera plane: finest, unless nothing of it is on screen.
                    if seen {
                        return 0;
                    }
                    continue;
                }
                if !seen {
                    continue;
                }
                let q = pts.map(|p| p * scale);
                let area = 0.5 * (cross(q[1] - q[0], q[2] - q[0]).abs() + cross(q[2] - q[0], q[3] - q[0]).abs());
                dens = dens.max(area / 256.0);
                edge = edge.max((0..4).map(|k| (q[(k + 1) % 4] - q[k]).length()).fold(0.0, f32::max) / 16.0);
            }
        }
        let (area_px, edge_px) = lod_limits();
        let mut k = 0u8;
        while (k as usize) + 1 < LODS {
            let s = (2 << k) as f32;
            if s * dens.sqrt() <= area_px && s * edge <= edge_px {
                k += 1;
            } else {
                break;
            }
        }
        k
    }

    /// Is any part of the box inside the view frustum (the four side planes and in front)?
    fn box_visible(vp: &glam::Mat4, b: &[f32; 6]) -> bool {
        let corners: [glam::Vec4; 8] = std::array::from_fn(|k| {
            let c = Vec3::new(b[if k & 1 == 0 { 0 } else { 3 }], b[if k & 2 == 0 { 1 } else { 4 }], b[if k & 4 == 0 { 2 } else { 5 }]);
            *vp * c.extend(1.0)
        });
        let outside = |f: fn(&glam::Vec4) -> f32| corners.iter().all(|c| f(c) < 0.0);
        !(outside(|c| c.w + c.x) || outside(|c| c.w - c.x) || outside(|c| c.w + c.y) || outside(|c| c.w - c.y) || outside(|c| c.w))
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
        for (off, bytes) in self.pool.pending.drain(..) {
            dev.write_buffer(self.pool_buf.id, off, &bytes)?;
        }
        Ok(())
    }

    fn poll(&mut self, _dev: &dyn Device) {}

    /// Needs `FrameData.view_proj` and `.light` already in `fd`: the view's nodes get an index LOD
    /// from their projected cell size and are grouped by it; each cascade draws only the nodes
    /// whose bounds touch its box, at the LOD its texel size allows.
    fn prepare(&mut self, dev: &dyn Device, _pools: &InstancePools, width: u32, height: u32, fd: &mut [u32]) -> Result<(), ed_gpu::Error> {
        let cur = self.cur;
        let mat_rec = gpu::bytes(&self.material_words());
        if self.views[cur].entries.is_empty() {
            self.views[cur].ranges = [(0, 0); 4];
            self.views[cur].vis.clear();
            self.views[cur].groups = [[(0, 0); LODS]; 5];
            self.views[cur].shells.clear();
            return Ok(());
        }
        let mat = |at: usize| glam::Mat4::from_cols_array(&std::array::from_fn(|k| f32::from_bits(fd.get(at + k).copied().unwrap_or(0))));
        let vp = mat(0);
        let all = std::mem::take(&mut self.views[cur].entries);
        let bounds = std::mem::take(&mut self.views[cur].bounds);
        // Morph-free nodes: the farthest corner of the box is inside the morph band's start.
        let all: Vec<TerrainDraw> = all
            .into_iter()
            .zip(&bounds)
            .map(|(mut e, b)| {
                let far = (0..8).map(|k| Vec3::new(b[if k & 1 == 0 { 0 } else { 3 }], b[if k & 2 == 0 { 1 } else { 4 }], b[if k & 4 == 0 { 2 } else { 5 }]).length()).fold(0.0f32, f32::max);
                if far <= e.morph[0] {
                    e.slots[2] |= F_NOMORPH;
                } else {
                    e.slots[2] &= !F_NOMORPH;
                }
                e
            })
            .collect();
        let flat = std::env::var_os("ED_TERRAIN_NOLOD").is_some();
        let nocull = std::env::var_os("ED_TERRAIN_NOCULL").is_some();
        let mut main: Vec<TerrainDraw> = all.iter().zip(&bounds).filter(|(_, b)| nocull || Self::box_visible(&vp, b)).map(|(e, _)| *e).collect();
        let want: Vec<u8> = main.iter().map(|d| if flat { 0 } else { self.main_lod(d, &vp, width as f32, height as f32) }).collect();
        let lods = assign_lods(&mut main, &want);
        let mut groups = [[(0u32, 0u32); LODS]; 5];
        groups[0] = sort_by_lod(&lods, &mut main);
        // Grass shells: the view's nodes on the map's face inside the shell range, at the
        // view's index LOD (the shells follow exactly the drawn ground).
        let mut shell_draws: Vec<(u8, TerrainDraw)> = Vec::new();
        let mut face_u = [0.0f32; 3];
        let mut shell_fd: Option<[u32; gpu::SHELL_WORDS]> = Some([0; gpu::SHELL_WORDS]);
        if let Some(sg) = &self.grass {
            for (l, g) in groups[0].iter().enumerate() {
                for d in &main[g.0 as usize..(g.0 + g.1) as usize] {
                    if d.id.map(|i| i.face()) != Some(sg.face) {
                        continue;
                    }
                    face_u = [d.fu[0], d.fu[1], d.fu[2]];
                    if sg.shells > 0 && sg.maps.is_some() && shell_draws.len() < SHELL_MAX_DRAWS && self.draw_near_m(d) < sg.range {
                        shell_draws.push((l as u8, *d));
                    }
                }
            }
            let (_, types_at, map_at, _) = shell_layout(self.config.max_draws);
            let flags = if shell_draws.is_empty() { 0 } else { 1 } | if sg.tint > 0.0 && sg.maps.is_some() { 2 } else { 0 } | if sg.maps.is_some() { 4 } else { 0 };
            let f = |x: f32| x.to_bits();
            let w: [u32; gpu::SHELL_WORDS] = [
                flags, types_at, map_at, sg.shells.min(SHELL_MAX),
                sg.offsets[0], sg.offsets[1], sg.offsets[2], sg.offsets[3],
                f(sg.centre[0]), f(sg.centre[1]), f(sg.range), f(sg.tint),
                f(face_u[0]), f(face_u[1]), f(face_u[2]), f(sg.h_max),
                f(sg.cells[0]), f(sg.cells[1]), f(SHELL_MAP_N as f32), 0,
            ];
            shell_fd = Some(w);
        }
        // The carpet table's first word (0: none), in the frame's shell block.
        if let Some(w) = shell_fd.as_mut() {
            w[19] = if self.config.ground_pages && self.carpet.is_some() { carpet_word(self.config.max_draws) as u32 } else { 0 };
        }
        let mut words: Vec<u32> = main.iter().flat_map(|e| e.words()).collect();
        let mut next = main.len() as u32;
        let mut ranges = [(0, 0); 4];
        for c in 0..4 {
            let m = mat(gpu::FD_LIGHT + c * 16);
            let texel = cascade_texel_m(&m, self.shadow_tile);
            let ortho = m.row(3) == glam::Vec4::W;
            let mut list: Vec<TerrainDraw> = all.iter().zip(&bounds).filter(|(_, b)| cascade_touches(&m, b)).map(|(e, _)| *e).collect();
            let want: Vec<u8> = list.iter().map(|d| if flat || !ortho { 0 } else { cascade_lod(d, texel, c) }).collect();
            let lods = assign_lods(&mut list, &want);
            groups[c + 1] = sort_by_lod(&lods, &mut list);
            let first = next;
            for e in &list {
                words.extend_from_slice(&e.words());
                next += 1;
            }
            for g in groups[c + 1].iter_mut() {
                g.0 += first;
            }
            ranges[c] = (first, next - first);
        }
        let v = &mut self.views[cur];
        v.entries = all;
        v.bounds = bounds;
        v.vis = main;
        v.ranges = ranges;
        if let Some(w) = shell_fd {
            fd[gpu::FD_SHELL..gpu::FD_SHELL + gpu::SHELL_WORDS].copy_from_slice(&w);
        }
        v.groups = groups;
        v.shells = shell_draws.iter().map(|(l, _)| *l).collect();
        let bytes = gpu::bytes(&words);
        let at = self.config.max_draws as usize * 5 * DRAW_WORDS * 4;
        let (draw_base, types_at, map_at, _) = shell_layout(self.config.max_draws);
        let shell_bytes = gpu::bytes(&shell_draws.iter().flat_map(|(_, d)| d.words()).collect::<Vec<u32>>());
        let grass = self.grass.as_ref();
        let type_bytes = grass.map(|g| gpu::bytes(&g.types.iter().take(SHELL_MAX_TYPES).flat_map(|r| r.iter().map(|x| x.to_bits())).collect::<Vec<u32>>()));
        let maps = grass.and_then(|g| g.maps.clone());
        let carpet_bytes = self.carpet.as_ref().map(|c| gpu::bytes(&c.iter().flat_map(|r| r.iter().map(|x| x.to_bits())).collect::<Vec<u32>>()));
        let carpet_at = carpet_word(self.config.max_draws) * 4;
        dev.fill_frame_buffer(v.draws.id, &mut |dst| {
            dst[..bytes.len()].copy_from_slice(&bytes);
            dst[at..at + mat_rec.len()].copy_from_slice(&mat_rec);
            let sa = draw_base as usize * DRAW_WORDS * 4;
            dst[sa..sa + shell_bytes.len()].copy_from_slice(&shell_bytes);
            if let Some(tb) = &type_bytes {
                let ta = types_at as usize * SHELL_TYPE_FLOATS * 4;
                dst[ta..ta + tb.len()].copy_from_slice(tb);
            }
            if let Some(cb) = &carpet_bytes {
                dst[carpet_at..carpet_at + cb.len()].copy_from_slice(cb);
            }
            if let Some(m) = &maps {
                let ma = map_at as usize * 4;
                let n = m.len().min(2 * SHELL_MAP_N * SHELL_MAP_N);
                for (i, w) in m[..n].iter().enumerate() {
                    dst[ma + i * 4..ma + i * 4 + 4].copy_from_slice(&w.to_ne_bytes());
                }
            }
        })
    }

    fn record<'a>(&mut self, g: &mut FrameGraph<'a>, _frame_index: u64, inp: &SharedInputs, out: VisTargets) -> ProviderOutputs {
        if let Some(sh) = &out.shadow {
            self.shadow_tile = sh.tile;
        }
        let v = &mut self.views[self.cur];
        let n = v.vis.len() as u32;
        let draws = g.import_buffer("terrain draws", v.draws.id, v.draws.state, None);
        let pool = g.import_buffer("terrain page pool", self.pool_buf.id, self.pool_buf.state, Some(ResourceState::CopyDst));
        let index = g.import_buffer("terrain indices", self.index.id, self.index.state, None);
        v.imports = vec![(draws, 0), (pool, 1), (index, 2)];
        // The material arrays: read (by the resolve) from the terrain pass on, so the graph moves
        // them out of their upload state on the first frame.
        let mat_res: Vec<Res> = match &self.mats {
            Some(t) => {
                let info = crate::graph::TextureInfo { format: ed_gpu::Format::Rgba8Unorm, width: t.size, height: t.size };
                let a = g.import_texture("terrain albedo", t.albedo.0, crate::graph::TextureInfo { format: ed_gpu::Format::Rgba8Srgb, ..info }, t.state, None);
                let b = g.import_texture("terrain normal", t.normal.0, info, t.state, None);
                v.imports.push((a, 3));
                v.imports.push((b, 3));
                vec![a, b]
            }
            None => Vec::new(),
        };
        let consts = |cascade: u32, base: u32| gpu::constants(&[inp.frame.1, v.draws.bindless.0, self.pool_buf.bindless.0, cascade, base]);
        let (idx_id, pipe, spipe) = (self.index.id, self.raster, self.shadow);
        // One instanced draw per (list, index LOD) group: (constants, first index, index count, instances).
        let groups_of = |list: usize, cascade: u32| -> Vec<(Consts, u32, u32, u32)> {
            v.groups[list].iter().enumerate().filter(|(_, g)| g.1 > 0).map(|(l, g)| (consts(cascade, g.0), lod_index_offset(l as u32), lod_tris(l as u32) * 3, g.1)).collect()
        };
        let main_groups = groups_of(0, 0);
        let clear = self.config.clear_targets;
        let load = if clear { LoadOp::Clear([0.0; 4]) } else { LoadOp::Load };
        let read = ResourceState::ShaderRead;
        let mut tp = g.add_pass("terrain", PassKind::Raster);
        for r in &mat_res {
            tp = tp.read(*r, read);
        }
        tp.read(inp.frame.0, read)
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
                for (cv, first, count, inst) in &main_groups {
                    ctx.enc.set_draw_constants(cv);
                    ctx.enc.draw_indexed(*first..*first + *count, 0, 0..*inst);
                }
            });
        // Grass shells over the drawn ground (styles C and D): one draw per node, shells as
        // instances, no index buffer (a triangle culls itself whole in `vs_shell`).
        let shells_n = self.grass.as_ref().map_or(0, |g| g.shells.min(SHELL_MAX));
        if shells_n > 0 && !v.shells.is_empty() {
            let base = shell_layout(self.config.max_draws).0;
            let sdraws: Vec<(Consts, u32)> = v.shells.iter().enumerate().map(|(i, l)| (consts(*l as u32, base + i as u32), lod_grid_tris(*l as u32) * 3)).collect();
            let shp = self.shell;
            g.add_pass("terrain_shells", PassKind::Raster)
                .read(inp.frame.0, read)
                .read(draws, read)
                .read(pool, read)
                .color(out.vis, LoadOp::Load, StoreOp::Store)
                .depth(out.depth, LoadOp::Load, StoreOp::Store, true)
                .exec(move |ctx| {
                    ctx.enc.set_render_pipeline(shp);
                    for (cv, verts) in &sdraws {
                        ctx.enc.set_draw_constants(cv);
                        ctx.enc.draw(0..*verts, 0..shells_n);
                    }
                });
        }
        // No caster in any cascade: no pass (unless it has to clear the atlas).
        let casters = v.ranges.iter().any(|r| r.1 > 0);
        if let Some(sh) = out.shadow.filter(|_| casters || clear) {
            let (cascades, tile) = (sh.cascades.min(4), sh.tile);

            let cs: Vec<Vec<(Consts, u32, u32, u32)>> = (0..cascades).map(|c| groups_of(c as usize + 1, c)).collect();
            g.add_pass("terrain_shadow", PassKind::Raster)
                .read(inp.frame.0, read)
                .read(draws, read)
                .read(pool, read)
                .read(index, read)
                .depth(sh.atlas, load, StoreOp::Store, true)
                .exec(move |ctx| {
                    if cs.iter().all(|g| g.is_empty()) {
                        return;
                    }
                    ctx.enc.set_index_buffer(idx_id, 0);
                    ctx.enc.set_render_pipeline(spipe);
                    for (c, gs) in cs.iter().enumerate() {
                        if gs.is_empty() {
                            continue;
                        }
                        let c = c as u32;
                        let (x, y) = ((c % 2) * tile, (c / 2) * tile);
                        ctx.enc.set_viewport(x as f32, y as f32, tile as f32, tile as f32);
                        ctx.enc.set_scissor(x, y, tile, tile);
                        for (cc, first, count, inst) in gs {
                            ctx.enc.set_draw_constants(cc);
                            ctx.enc.draw_indexed(*first..*first + *count, 0, 0..*inst);
                        }
                    }
                });
        }
        ProviderOutputs { meshes: (draws, v.draws.bindless.0), state: (pool, self.pool_buf.bindless.0), hzb: None, imp: None, grass: None }
    }

    fn finish(&mut self, plan: &Plan) {
        let v = &mut self.views[self.cur];
        for &(r, which) in &v.imports {
            let s = plan.final_states[r.index()];
            match which {
                0 => v.draws.state = s,
                1 => self.pool_buf.state = s,
                3 => {
                    if let Some(t) = &mut self.mats {
                        t.state = s;
                    }
                }
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
        &["terrain", "terrain_shells", "terrain_shadow"]
    }

    fn destroy(mut self: Box<Self>, dev: &dyn Device) {
        self.clear_materials(dev);
        dev.destroy_render_pipeline(self.raster);
        dev.destroy_render_pipeline(self.shadow);
        dev.destroy_render_pipeline(self.shell);
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

    #[test]
    fn landcover_origin_places_a_node_in_its_level_12_page() {
        let b = ed_geo::BodyId(0);
        // At and above level 12 a node reads its own page.
        assert_eq!(landcover_origin(NodeId::new(b, 2, 12, 5, 9).unwrap()), (0, 1.0));
        assert_eq!(landcover_origin(NodeId::new(b, 2, 8, 5, 9).unwrap()), (0, 1.0));
        // Level 13, odd x, even y: the right half of the ancestor's page, half scale.
        let (o, s) = landcover_origin(NodeId::new(b, 2, 13, 11, 18).unwrap());
        assert_eq!((o & 0xFFFF, o >> 16, s), (128 * 128, 0, 0.5));
        // Level 20: the last child is 255/256 of the way across, 1/256 scale.
        let id = NodeId::new(b, 2, 20, (5 << 8) + 255, (9 << 8) + 3).unwrap();
        let (o, s) = landcover_origin(id);
        assert_eq!((o & 0xFFFF, o >> 16), (255 * 128, 3 * 128));
        assert_eq!(s, 1.0 / 256.0);
    }
    use ed_geo::BodyId;
    use glam::DQuat;

    /// The crown colour table is one table: the resolve shader's `T_CROWN_*` match it.
    #[test]
    fn crown_albedo_matches_shader() {
        let src = include_str!("../shaders/resolve.slang");
        for (name, row) in [("T_CROWN_BROAD", CROWN_ALBEDO[0]), ("T_CROWN_CONIFER", CROWN_ALBEDO[1])] {
            let line = src.lines().find(|l| l.contains(&format!("float3 {name} ="))).expect(name);
            let args = line.split("float3(").nth(1).unwrap().split(')').next().unwrap();
            let v: Vec<f32> = args.split(',').map(|x| x.trim().parse().unwrap()).collect();
            assert_eq!(v, row.to_vec(), "{name}");
        }
    }

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
