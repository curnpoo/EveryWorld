//! Building parts library: windows, doors, stoops, shopfronts, balconies,
//! shutters, AC units, fire escapes, water tanks, chimneys, dishes, rooftop
//! gear, garage doors, porches, mailboxes, fences.
//!
//! Every part is modeled (profiles, chamfers, lathes, sweeps), placed in a
//! facade `Basis` (x along the wall, y up, z out of the wall), and written
//! into the per-material buckets of `Out`.

use crate::archkit::{Balcony, Lintel, RoofMat, WallMat, WindowDesign};
use crate::mesh::{MeshData, rgb, scale_color};
use crate::meshkit::{self as mk, Basis, Opening, profiles};
use glam::{Vec2, Vec3};
use std::collections::HashMap;

/// Material buckets the game maps to real materials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mat {
    Wall(WallMat),
    Roof(RoofMat),
    Glass,
    /// Painted / stone trim, vertex colored, matte.
    Trim,
    /// Dark iron and steel.
    Metal,
    /// Awnings, umbrellas.
    Fabric,
    /// Paving, driveways, steps.
    Paving,
    /// Plants on roofs and balconies.
    Plant,
}

#[derive(Debug, Default)]
pub struct Out {
    pub meshes: HashMap<Mat, MeshData>,
}

impl Out {
    pub fn m(&mut self, k: Mat) -> &mut MeshData {
        self.meshes.entry(k).or_default()
    }
    pub fn triangle_count(&self) -> usize {
        self.meshes.values().map(MeshData::triangle_count).sum()
    }
    pub fn append(&mut self, other: Out) {
        for (k, v) in other.meshes {
            self.m(k).append(&v);
        }
    }
}

const IRON: u32 = 0x23262b;
const GLASS_TINT: u32 = 0x9fb4c0;

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

fn blk(o: &mut Out, k: Mat, b: &Basis, min: Vec3, max: Vec3, bevel: f32, c: [f32; 4]) {
    mk::block(o.m(k), b, min, max, bevel, c);
}

/// A thin square bar between two points (railings, mullions, ladders).
pub fn bar(o: &mut Out, k: Mat, a: Vec3, b: Vec3, s: f32, up_hint: Vec3, c: [f32; 4]) {
    let d = (b - a).normalize_or(Vec3::Y);
    // Choose a sweep "up" that is not parallel to the bar.
    let up = if d.dot(up_hint).abs() > 0.9 { d.any_orthonormal_vector() } else { up_hint.reject_from_normalized(d).normalize() };
    // Sweep a centered square profile; its local y axis is `up`.
    let h = s * 0.5;
    let prof = [Vec2::new(-h, -h), Vec2::new(h, -h), Vec2::new(h, h), Vec2::new(-h, h), Vec2::new(-h, -h)];
    mk::sweep(o.m(k), &prof, &[a, b], false, up, c);
}

// ---------------------------------------------------------------- windows

/// Everything inside and around one window opening. `b` is the facade basis;
/// the opening is in facade (x, y) coordinates.
pub fn window(o: &mut Out, b: &Basis, op: &Opening, d: &WindowDesign, trim: [f32; 4], shutter: Option<[f32; 4]>, wall_mat: WallMat) {
    let r = d.recess;
    let frame_c = rgb(d.frame);
    let outline = op.outline();
    // Frame around the glass, set back in the reveal.
    mk::frame_along(o.m(Mat::Trim), b, &outline, -r + 0.03, 0.08, 0.05, frame_c);
    // Glass, slightly behind the frame.
    let glass: Vec<Vec3> = outline.iter().map(|p| b.p(p.x, p.y, -r)).collect();
    mk::polygon(o.m(Mat::Glass), &glass, &[], b.z, 1.0, rgb(GLASS_TINT));
    // Mullions and transoms.
    let (x0, y0, w, h) = (op.x0, op.y0, op.w, op.h);
    let mz = -r + 0.02;
    for i in 1..d.cols {
        let x = x0 + w * i as f32 / d.cols as f32;
        bar(o, Mat::Trim, b.p(x, y0, mz), b.p(x, y0 + h * 0.98, mz), 0.04, b.z, frame_c);
    }
    let glazed_h = if matches!(op.arch, mk::Arch::Flat) { h } else { h * 0.72 };
    for j in 1..d.rows {
        let y = y0 + glazed_h * j as f32 / d.rows as f32;
        bar(o, Mat::Trim, b.p(x0 + 0.02, y, mz), b.p(x0 + w - 0.02, y, mz), 0.035, b.y, frame_c);
    }
    // Sill: a sloped stone/painted ledge with drip.
    let sill_c = if matches!(wall_mat, WallMat::Brick | WallMat::Stone) { rgb(0xd8d0c0) } else { trim };
    blk(o, Mat::Trim, b, v(x0 - 0.07, y0 - 0.07, -r), v(x0 + w + 0.07, y0, 0.07), 0.015, sill_c);
    // Lintel styles.
    let top = y0 + h;
    match d.lintel {
        Lintel::None => {}
        Lintel::Flat => blk(o, Mat::Trim, b, v(x0 - 0.1, top, -0.02), v(x0 + w + 0.1, top + 0.2, 0.03), 0.01, sill_c),
        Lintel::Keystone => {
            blk(o, Mat::Trim, b, v(x0 - 0.12, top, -0.02), v(x0 + w + 0.12, top + 0.18, 0.025), 0.01, sill_c);
            blk(o, Mat::Trim, b, v(x0 + w * 0.5 - 0.1, top - 0.04, -0.02), v(x0 + w * 0.5 + 0.1, top + 0.3, 0.06), 0.015, sill_c);
        }
        Lintel::Hood => {
            // Projecting cornice hood with a small cornice profile.
            let path = [b.p(x0 - 0.18, top + 0.12, 0.0), b.p(x0 + w + 0.18, top + 0.12, 0.0)];
            mk::sweep(o.m(Mat::Trim), &profiles::cornice(0.16, 0.2), &path, false, b.y, trim);
            blk(o, Mat::Trim, b, v(x0 - 0.12, top, -0.02), v(x0 + w + 0.12, top + 0.13, 0.03), 0.01, trim);
        }
    }
    // Stone surround (architrave) proud of the wall.
    if d.surround {
        mk::frame_along(o.m(Mat::Trim), b, &outline, 0.0, 0.16, 0.035, trim);
    }
    // Iron grille in front of the glass.
    if d.grille {
        let gz = -0.03;
        let n = ((w / 0.13) as usize).max(2);
        for i in 1..n {
            let x = x0 + w * i as f32 / n as f32;
            bar(o, Mat::Metal, b.p(x, y0 + 0.02, gz), b.p(x, y0 + glazed_h - 0.02, gz), 0.018, b.z, rgb(IRON));
        }
        for y in [y0 + 0.12, y0 + glazed_h * 0.5, y0 + glazed_h - 0.12] {
            bar(o, Mat::Metal, b.p(x0 + 0.02, y, gz), b.p(x0 + w - 0.02, y, gz), 0.02, b.y, rgb(IRON));
        }
    }
    // Open louvered shutters on both sides.
    if let Some(sc) = shutter {
        let sw = w * 0.5;
        for (sx0, sx1) in [(x0 - sw - 0.02, x0 - 0.02), (x0 + w + 0.02, x0 + w + sw + 0.02)] {
            blk(o, Mat::Trim, b, v(sx0, y0, 0.01), v(sx1, y0 + glazed_h, 0.05), 0.008, scale_color(sc, 0.9));
            // Louvers: angled slats across the panel.
            let n = ((glazed_h - 0.16) / 0.065) as usize;
            for k in 0..n {
                let y = y0 + 0.08 + k as f32 * 0.065;
                let p = [b.p(sx0 + 0.05, y, 0.05), b.p(sx1 - 0.05, y, 0.05), b.p(sx1 - 0.05, y + 0.05, 0.075), b.p(sx0 + 0.05, y + 0.05, 0.075)];
                o.m(Mat::Trim).quad(p, [[0.0; 2]; 4], sc);
            }
        }
    }
}

/// A planter box under a window with a mound of flowers.
pub fn flower_box(o: &mut Out, b: &Basis, x0: f32, w: f32, y: f32, seed: u64) {
    blk(o, Mat::Trim, b, v(x0 - 0.05, y - 0.22, 0.05), v(x0 + w + 0.05, y - 0.02, 0.3), 0.02, rgb(0x6b4a2f));
    let colors = [0xd62828, 0xf77f00, 0xe5383b, 0xff70a6, 0xfcbf49, 0xc77dff];
    let n = ((w / 0.18) as usize).max(2);
    for i in 0..n {
        let x = x0 + (i as f32 + 0.5) * w / n as f32;
        let c = rgb(colors[(crate::hash01(seed, i as u64) * colors.len() as f32) as usize % colors.len()]);
        bush(o, b.p(x, y + 0.02, 0.17), 0.13, 0.11, c, seed ^ i as u64);
    }
}

/// A lumpy foliage mass (noise-displaced, multi-lobed), for planters and flowers.
pub fn bush(o: &mut Out, center: Vec3, r: f32, h: f32, c: [f32; 4], seed: u64) {
    let m = o.m(Mat::Plant);
    let (rings, seg) = (5usize, 9usize);
    let base = m.positions.len() as u32;
    for j in 0..=rings {
        let phi = j as f32 / rings as f32 * std::f32::consts::PI;
        for i in 0..=seg {
            let th = i as f32 / seg as f32 * std::f32::consts::TAU;
            let n = Vec3::new(phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin());
            let jitter = 0.78 + 0.44 * crate::hash01(seed, (j * 31 + i % seg) as u64);
            let p = center + Vec3::new(n.x * r, (n.y * 0.5 + 0.5) * h, n.z * r) * jitter;
            let shade = 0.7 + 0.3 * (n.y * 0.5 + 0.5);
            m.vertex(p.to_array(), n.to_array(), [0.0; 2], scale_color(c, shade));
        }
    }
    let w = seg as u32 + 1;
    for j in 0..rings as u32 {
        for i in 0..seg as u32 {
            let a = base + j * w + i;
            m.tri(a, a + 1, a + w);
            m.tri(a + 1, a + w + 1, a + w);
        }
    }
}

// ---------------------------------------------------------------- doors

pub fn door(o: &mut Out, b: &Basis, op: &Opening, color: [f32; 4], trim: [f32; 4]) {
    let (x0, y0, w, h) = (op.x0, op.y0, op.w, op.h);
    let r = 0.2;
    let outline = op.outline();
    mk::frame_along(o.m(Mat::Trim), b, &outline, -0.02, 0.12, 0.06, trim);
    let leaf_h = if matches!(op.arch, mk::Arch::Flat) { h } else { h * 0.72 };
    // Fanlight glass in the arch.
    if leaf_h < h {
        let fan: Vec<Vec3> = outline.iter().map(|p| b.p(p.x, p.y.max(y0 + leaf_h), -r + 0.02)).collect();
        mk::polygon(o.m(Mat::Glass), &fan, &[], b.z, 1.0, rgb(GLASS_TINT));
        bar(o, Mat::Trim, b.p(x0, y0 + leaf_h, -r + 0.05), b.p(x0 + w, y0 + leaf_h, -r + 0.05), 0.06, b.y, trim);
    }
    // Leaf with raised panels (double door if wide).
    let leaves = if w > 1.5 { 2 } else { 1 };
    let lw = w / leaves as f32;
    for l in 0..leaves {
        let lx = x0 + l as f32 * lw;
        blk(o, Mat::Trim, b, v(lx + 0.01, y0, -r - 0.04), v(lx + lw - 0.01, y0 + leaf_h, -r), 0.01, color);
        for (py0, py1) in [(0.12, 0.42), (0.52, 0.94)] {
            for (px0, px1) in [(0.12, 0.46), (0.54, 0.88)] {
                blk(o, Mat::Trim, b, v(lx + lw * px0, y0 + leaf_h * py0, -r), v(lx + lw * px1, y0 + leaf_h * py1, -r + 0.025), 0.012, scale_color(color, 1.12));
            }
        }
        // Handle.
        let hx = if leaves == 2 && l == 0 { lx + lw - 0.1 } else { lx + 0.1 };
        mk::lathe(o.m(Mat::Metal), b.p(hx, y0 + 1.0, -r + 0.02), b.z, &[Vec2::new(0.025, 0.0), Vec2::new(0.025, 0.04), Vec2::new(0.012, 0.05), Vec2::new(0.0, 0.07)], 8, rgb(0xb08d57));
    }
    // Threshold step.
    blk(o, Mat::Paving, b, v(x0 - 0.1, y0 - 0.16, -r), v(x0 + w + 0.1, y0, 0.3), 0.02, rgb(0xbdb6aa));
}

/// Brownstone stoop: steps up to a raised door with iron railings.
pub fn stoop(o: &mut Out, b: &Basis, x_center: f32, ground_y: f32, door_y: f32, width: f32, stone: [f32; 4]) {
    let rise = (door_y - ground_y).max(0.2);
    let n = (rise / 0.18).ceil() as usize;
    let tread = 0.3;
    let depth = n as f32 * tread + 1.0;
    // Landing.
    blk(o, Mat::Paving, b, v(x_center - width * 0.5, door_y - 0.2, 0.0), v(x_center + width * 0.5, door_y, 1.0), 0.02, stone);
    for k in 0..n {
        let y = door_y - (k as f32 + 1.0) * rise / n as f32;
        let z0 = 1.0 + k as f32 * tread;
        blk(o, Mat::Paving, b, v(x_center - width * 0.5, ground_y - 0.1, z0), v(x_center + width * 0.5, y + 0.0, z0 + tread + 0.02), 0.015, stone);
    }
    // Cheek walls and railings.
    for s in [-1.0f32, 1.0] {
        let x = x_center + s * (width * 0.5 + 0.08);
        let a = b.p(x, door_y + 0.9, 0.05);
        let c = b.p(x, ground_y + 0.95, depth);
        bar(o, Mat::Metal, a, c, 0.04, b.y, rgb(IRON));
        let posts = ((depth / 0.25) as usize).max(2);
        for i in 0..=posts {
            let t = i as f32 / posts as f32;
            let z = 0.05 + t * (depth - 0.05);
            let y_step = door_y - (((z - 1.0) / tread).max(0.0).floor() * rise / n as f32).min(rise);
            let top = a.lerp(c, t);
            bar(o, Mat::Metal, b.p(x, y_step, z), top, 0.022, b.z, rgb(IRON));
        }
        // Newel post at the bottom.
        mk::lathe(o.m(Mat::Metal), b.p(x, ground_y, depth), b.y, &[Vec2::new(0.06, 0.0), Vec2::new(0.06, 0.1), Vec2::new(0.035, 0.15), Vec2::new(0.035, 1.0), Vec2::new(0.06, 1.05), Vec2::new(0.0, 1.15)], 10, rgb(IRON));
    }
}

/// Glazed shopfront with kick plate, sign band and (optionally) a fabric awning.
pub fn shopfront(o: &mut Out, b: &Basis, op: &Opening, frame: [f32; 4], sign: [f32; 4], awning: Option<[f32; 4]>, seed: u64) {
    let (x0, y0, w, h) = (op.x0, op.y0, op.w, op.h);
    let r = 0.12;
    let outline = op.outline();
    mk::frame_along(o.m(Mat::Trim), b, &outline, -0.01, 0.14, 0.05, frame);
    let glass_y0 = y0 + 0.45;
    // Kick plate.
    blk(o, Mat::Trim, b, v(x0, y0, -r - 0.05), v(x0 + w, glass_y0, -r), 0.01, scale_color(frame, 0.8));
    let g = [b.p(x0, glass_y0, -r), b.p(x0 + w, glass_y0, -r), b.p(x0 + w, y0 + h, -r), b.p(x0, y0 + h, -r)];
    mk::polygon(o.m(Mat::Glass), &g, &[], b.z, 1.0, rgb(0xb0c4cc));
    // Mullions; one bay is a door.
    let n = ((w / 1.2) as usize).max(2);
    for i in 1..n {
        let x = x0 + w * i as f32 / n as f32;
        bar(o, Mat::Trim, b.p(x, y0, -r + 0.03), b.p(x, y0 + h, -r + 0.03), 0.05, b.z, frame);
    }
    bar(o, Mat::Trim, b.p(x0, y0 + h * 0.8, -r + 0.03), b.p(x0 + w, y0 + h * 0.8, -r + 0.03), 0.04, b.y, frame);
    // Sign band above, projecting.
    blk(o, Mat::Trim, b, v(x0 - 0.1, y0 + h + 0.05, -0.02), v(x0 + w + 0.1, y0 + h + 0.6, 0.12), 0.02, sign);
    // Fake lettering: a few light bars on the sign.
    let letters = 3 + (crate::hash01(seed, 3) * 6.0) as usize;
    for k in 0..letters {
        let lx = x0 + w * 0.15 + k as f32 * w * 0.7 / letters as f32;
        blk(o, Mat::Trim, b, v(lx, y0 + h + 0.2, 0.12), v(lx + w * 0.5 / letters as f32, y0 + h + 0.45, 0.14), 0.0, rgb(0xf4f2ec));
    }
    if let Some(c) = awning {
        awning_at(o, b, x0 - 0.05, w + 0.1, y0 + h + 0.02, 1.2, c);
    }
}

/// Curved fabric awning with a scalloped valance, projecting `depth` meters.
pub fn awning_at(o: &mut Out, b: &Basis, x0: f32, w: f32, y_top: f32, depth: f32, c: [f32; 4]) {
    // Profile in (z out, y) along the awning: a gentle convex curve.
    let segs = 6;
    let curve: Vec<Vec2> = (0..=segs)
        .map(|i| {
            let t = i as f32 / segs as f32;
            Vec2::new(depth * t, -0.55 * t * t - 0.15 * t)
        })
        .collect();
    let m = o.m(Mat::Fabric);
    let stripes = ((w / 0.45) as usize).max(2);
    for s in 0..stripes {
        let xa = x0 + w * s as f32 / stripes as f32;
        let xb = x0 + w * (s + 1) as f32 / stripes as f32;
        let col = if s % 2 == 0 { c } else { scale_color(c, 0.35).map(|x| x + 0.55) };
        let col = [col[0].min(1.0), col[1].min(1.0), col[2].min(1.0), 1.0];
        for i in 0..segs {
            let (p, q) = (curve[i], curve[i + 1]);
            let quad = [b.p(xb, y_top + p.y, p.x), b.p(xa, y_top + p.y, p.x), b.p(xa, y_top + q.y, q.x), b.p(xb, y_top + q.y, q.x)];
            m.quad(quad, [[0.0; 2]; 4], col);
            m.quad([quad[1], quad[0], quad[3], quad[2]], [[0.0; 2]; 4], scale_color(col, 0.7));
        }
        // Scalloped valance.
        let end = curve[segs];
        let yv = y_top + end.y;
        let tri = [b.p(xa, yv, end.x), b.p(xb, yv, end.x), b.p((xa + xb) * 0.5, yv - 0.22, end.x)];
        m.triangle([tri[1], tri[0], tri[2]], [[0.0; 2]; 3], col);
        m.triangle(tri, [[0.0; 2]; 3], scale_color(col, 0.7));
        m.quad([b.p(xb, yv, end.x), b.p(xa, yv, end.x), b.p(xa, yv - 0.08, end.x), b.p(xb, yv - 0.08, end.x)].map(|p| p), [[0.0; 2]; 4], col);
    }
    // Side arms.
    for x in [x0 + 0.03, x0 + w - 0.03] {
        bar(o, Mat::Metal, b.p(x, y_top - 0.05, 0.02), b.p(x, y_top + curve[segs].y, depth), 0.025, b.y, rgb(IRON));
    }
}

/// Garage door: framed opening with horizontal ribbed panels.
pub fn garage_door(o: &mut Out, b: &Basis, op: &Opening, color: [f32; 4], trim: [f32; 4]) {
    let (x0, y0, w, h) = (op.x0, op.y0, op.w, op.h);
    mk::frame_along(o.m(Mat::Trim), b, &op.outline(), -0.02, 0.12, 0.05, trim);
    let n = 4;
    for k in 0..n {
        let ya = y0 + h * k as f32 / n as f32 + 0.01;
        let yb = y0 + h * (k + 1) as f32 / n as f32 - 0.01;
        blk(o, Mat::Trim, b, v(x0 + 0.02, ya, -0.14), v(x0 + w - 0.02, yb, -0.1), 0.015, color);
        // Raised panels in each section.
        let cols = (w / 0.9) as usize;
        for c in 0..cols {
            let xa = x0 + 0.12 + c as f32 * (w - 0.24) / cols as f32;
            let xb = xa + (w - 0.24) / cols as f32 - 0.1;
            blk(o, Mat::Trim, b, v(xa, ya + 0.08, -0.1), v(xb, yb - 0.08, -0.085), 0.008, scale_color(color, 1.08));
        }
    }
}

// ---------------------------------------------------------------- balconies

pub fn balcony(o: &mut Out, b: &Basis, kind: Balcony, x0: f32, w: f32, floor_y: f32, rail: [f32; 4], slab: [f32; 4], seed: u64) {
    let depth = match kind {
        Balcony::Juliet => 0.22,
        Balcony::Iron => 0.6,
        Balcony::Concrete => 1.1,
        Balcony::Timber => 1.0,
        Balcony::None => return,
    };
    let (xa, xb) = (x0, x0 + w);
    if kind != Balcony::Juliet {
        // Slab with chamfered edge, and brackets under it for iron/timber.
        blk(o, Mat::Trim, b, v(xa, floor_y - 0.16, 0.0), v(xb, floor_y, depth), 0.03, slab);
        if matches!(kind, Balcony::Iron | Balcony::Timber) {
            for x in [xa + 0.15, xb - 0.15] {
                let prof = [Vec2::new(0.0, 0.0), Vec2::new(0.06, 0.0), Vec2::new(0.06, 0.06), Vec2::new(0.0, 0.06), Vec2::new(0.0, 0.0)];
                let path = [b.p(x, floor_y - 0.55, 0.0), b.p(x, floor_y - 0.2, depth * 0.55), b.p(x, floor_y - 0.16, depth * 0.9)];
                mk::sweep(o.m(Mat::Trim), &prof, &path, false, b.x, slab);
            }
        }
    }
    let rail_h = 1.0;
    let corners = [b.p(xa, floor_y, 0.0), b.p(xa, floor_y, depth), b.p(xb, floor_y, depth), b.p(xb, floor_y, 0.0)];
    match kind {
        Balcony::Concrete => {
            // Solid parapet around three sides with a rounded cap.
            blk(o, Mat::Trim, b, v(xa, floor_y, depth - 0.12), v(xb, floor_y + rail_h, depth), 0.02, slab);
            blk(o, Mat::Trim, b, v(xa, floor_y, 0.0), v(xa + 0.12, floor_y + rail_h, depth), 0.02, slab);
            blk(o, Mat::Trim, b, v(xb - 0.12, floor_y, 0.0), v(xb, floor_y + rail_h, depth), 0.02, slab);
            if crate::hash01(seed, 31) < 0.3 {
                bush(o, b.p((xa + xb) * 0.5, floor_y + rail_h, depth - 0.06), 0.35, 0.3, rgb(0x4f8a3a), seed);
            }
        }
        Balcony::Timber => {
            let path = [corners[0] + b.y * rail_h, corners[1] + b.y * rail_h, corners[2] + b.y * rail_h, corners[3] + b.y * rail_h];
            for w2 in path.windows(2) {
                bar(o, Mat::Trim, w2[0], w2[1], 0.08, b.y, rail);
            }
            // Vertical boards with cut-out silhouettes (spaced planks).
            for w2 in corners.windows(2) {
                let len = w2[0].distance(w2[1]);
                let n = (len / 0.14) as usize;
                for i in 0..=n {
                    let p = w2[0].lerp(w2[1], i as f32 / n.max(1) as f32);
                    bar(o, Mat::Trim, p, p + b.y * rail_h, 0.07, b.z, scale_color(rail, 0.9));
                }
            }
            flower_box(o, &b.at(0.0, 0.0, depth - 0.02), xa + 0.1, w - 0.2, floor_y + rail_h + 0.2, seed);
        }
        Balcony::Iron | Balcony::Juliet => {
            let path = [corners[0] + b.y * rail_h, corners[1] + b.y * rail_h, corners[2] + b.y * rail_h, corners[3] + b.y * rail_h];
            for w2 in path.windows(2) {
                bar(o, Mat::Metal, w2[0], w2[1], 0.04, b.y, rgb(IRON));
            }
            for w2 in corners.windows(2) {
                let lower = [w2[0] + b.y * 0.08, w2[1] + b.y * 0.08];
                bar(o, Mat::Metal, lower[0], lower[1], 0.03, b.y, rgb(IRON));
                let len = w2[0].distance(w2[1]);
                let n = (len / 0.11) as usize;
                for i in 0..=n {
                    let p = w2[0].lerp(w2[1], i as f32 / n.max(1) as f32);
                    bar(o, Mat::Metal, p + b.y * 0.08, p + b.y * rail_h, 0.016, b.z, rgb(IRON));
                }
                // Decorative scroll band: a second rail with tiny diamonds.
                let band_y = rail_h * 0.82;
                bar(o, Mat::Metal, w2[0] + b.y * band_y, w2[1] + b.y * band_y, 0.02, b.y, rgb(IRON));
            }
        }
        Balcony::None => {}
    }
}

// ---------------------------------------------------------------- facade props

/// Window AC unit hanging below a window sill.
pub fn ac_unit(o: &mut Out, b: &Basis, x_center: f32, y_sill: f32) {
    let (w, h, d) = (0.62, 0.4, 0.55);
    let body = rgb(0xe3ded2);
    blk(o, Mat::Trim, b, v(x_center - w * 0.5, y_sill - h - 0.02, -0.05), v(x_center + w * 0.5, y_sill - 0.02, d), 0.03, body);
    // Front grille slats.
    for k in 0..7 {
        let y = y_sill - h + 0.04 + k as f32 * 0.045;
        blk(o, Mat::Trim, b, v(x_center - w * 0.42, y, d), v(x_center + w * 0.42, y + 0.02, d + 0.015), 0.0, scale_color(body, 0.75));
    }
    // Support bracket.
    for x in [x_center - w * 0.35, x_center + w * 0.35] {
        bar(o, Mat::Metal, b.p(x, y_sill - h - 0.4, 0.0), b.p(x, y_sill - h - 0.03, d * 0.9), 0.025, b.y, rgb(0x5a5f66));
    }
}

/// Wall-mounted AC condenser (Asia, Middle East).
pub fn condenser(o: &mut Out, b: &Basis, x_center: f32, y: f32) {
    let (w, h, d) = (0.85, 0.6, 0.32);
    let body = rgb(0xece8e0);
    blk(o, Mat::Trim, b, v(x_center - w * 0.5, y, 0.05), v(x_center + w * 0.5, y + h, 0.05 + d), 0.03, body);
    // Fan grille: concentric rings.
    let c = b.p(x_center - w * 0.15, y + h * 0.5, 0.05 + d + 0.005);
    for (r0, r1) in [(0.06, 0.08), (0.14, 0.16), (0.22, 0.24)] {
        mk::lathe(o.m(Mat::Metal), c, b.z, &[Vec2::new(r0, 0.0), Vec2::new(r1, 0.0), Vec2::new(r1, 0.01), Vec2::new(r0, 0.01)], 16, rgb(0x4a4d52));
    }
    for x in [x_center - w * 0.4, x_center + w * 0.4] {
        bar(o, Mat::Metal, b.p(x, y, 0.0), b.p(x, y, d + 0.05), 0.03, b.y, rgb(0x6a6e74));
    }
}

/// Fire escape on a facade: platforms at each upper floor spanning `x0..x1`,
/// with railings and stairs between landings, plus a drop ladder.
pub fn fire_escape(o: &mut Out, b: &Basis, x0: f32, x1: f32, floor_ys: &[f32]) {
    let iron = rgb(0x2d2f33);
    let depth = 1.05;
    for (k, &y) in floor_ys.iter().enumerate() {
        // Grated platform: frame + slats.
        blk(o, Mat::Metal, b, v(x0, y - 0.06, 0.0), v(x1, y, depth), 0.0, iron);
        for i in 0..((x1 - x0) / 0.12) as usize {
            let x = x0 + 0.06 + i as f32 * 0.12;
            blk(o, Mat::Metal, b, v(x, y, 0.02), v(x + 0.03, y + 0.015, depth - 0.02), 0.0, scale_color(iron, 1.2));
        }
        // Railing on three sides.
        let r = 1.0;
        let c = [b.p(x0, y, 0.02), b.p(x0, y, depth), b.p(x1, y, depth), b.p(x1, y, 0.02)];
        for w in c.windows(2) {
            bar(o, Mat::Metal, w[0] + b.y * r, w[1] + b.y * r, 0.035, b.y, iron);
            bar(o, Mat::Metal, w[0] + b.y * r * 0.5, w[1] + b.y * r * 0.5, 0.025, b.y, iron);
            let len = w[0].distance(w[1]);
            let n = (len / 0.12) as usize;
            for i in 0..=n {
                let p = w[0].lerp(w[1], i as f32 / n.max(1) as f32);
                bar(o, Mat::Metal, p, p + b.y * r, 0.018, b.z, iron);
            }
        }
        // Stair down to the platform below (diagonal stringers + treads).
        if k > 0 {
            let y_low = floor_ys[k - 1];
            let (sx0, sx1) = if k % 2 == 0 { (x0 + 0.2, x0 + (x1 - x0) * 0.6) } else { (x1 - (x1 - x0) * 0.6, x1 - 0.2) };
            let (top_x, bot_x) = if k % 2 == 0 { (sx0, sx1) } else { (sx1, sx0) };
            for zz in [0.3, 0.8] {
                bar(o, Mat::Metal, b.p(top_x, y, zz), b.p(bot_x, y_low, zz), 0.05, b.z, iron);
            }
            let steps = ((y - y_low) / 0.2) as usize;
            for s in 1..steps {
                let t = s as f32 / steps as f32;
                let x = top_x + (bot_x - top_x) * t;
                let yy = y + (y_low - y) * t;
                blk(o, Mat::Metal, b, v(x - 0.1, yy - 0.02, 0.3), v(x + 0.1, yy + 0.01, 0.8), 0.0, iron);
            }
        }
    }
    // Drop ladder below the lowest platform.
    if let Some(&y) = floor_ys.first() {
        let x = x1 - 0.4;
        for dx in [-0.2, 0.2] {
            bar(o, Mat::Metal, b.p(x + dx, y - 0.05, depth - 0.1), b.p(x + dx, y - 2.2, depth - 0.1), 0.03, b.z, iron);
        }
        for i in 0..7 {
            let yy = y - 0.3 - i as f32 * 0.3;
            bar(o, Mat::Metal, b.p(x - 0.2, yy, depth - 0.1), b.p(x + 0.2, yy, depth - 0.1), 0.025, b.y, iron);
        }
    }
}

/// Satellite dish on a short arm, facing roughly `aim` (in world space).
pub fn satellite_dish(o: &mut Out, pos: Vec3, aim: Vec3, up: Vec3, r: f32) {
    let aim = aim.normalize_or(Vec3::X);
    // Dish: shallow paraboloid lathed around the aim axis.
    let prof: Vec<Vec2> = (0..=6).map(|i| {
        let t = i as f32 / 6.0;
        Vec2::new(r * t, r * 0.25 * t * t)
    }).collect();
    mk::lathe(o.m(Mat::Trim), pos, -aim, &prof, 16, rgb(0xe9e7e1));
    mk::lathe(o.m(Mat::Trim), pos - aim * 0.005, aim, &prof, 16, rgb(0xcfccc4));
    // Feed arm and LNB.
    let feed = pos + aim * r * 0.9;
    bar(o, Mat::Metal, pos - up * r * 0.6, feed, 0.03, up, rgb(0x6a6e74));
    bar(o, Mat::Metal, pos, pos - up * r * 1.1, 0.05, aim, rgb(0x6a6e74));
    mk::lathe(o.m(Mat::Trim), feed, aim, &[Vec2::new(0.05, 0.0), Vec2::new(0.05, 0.08), Vec2::new(0.03, 0.12), Vec2::new(0.0, 0.12)], 8, rgb(0x3a3d42));
}

// ---------------------------------------------------------------- rooftop

/// NYC-style wooden water tank on a steel stand.
pub fn water_tank_nyc(o: &mut Out, base: Vec3, up: Vec3) {
    let wood = rgb(0x8a6a4a);
    let steel = rgb(0x4a4d52);
    let legs_h = 2.6;
    let r = 1.7;
    let side = up.any_orthonormal_vector();
    let side2 = up.cross(side);
    for (a, bq) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
        let p = base + (side * a + side2 * bq) * r * 0.7;
        bar(o, Mat::Metal, p, p + up * legs_h, 0.14, side, steel);
    }
    // Cross bracing.
    bar(o, Mat::Metal, base + (side + side2) * r * 0.7, base + (-side - side2) * r * 0.7 + up * legs_h, 0.05, up, steel);
    bar(o, Mat::Metal, base + (side - side2) * r * 0.7, base + (-side + side2) * r * 0.7 + up * legs_h, 0.05, up, steel);
    let t0 = base + up * legs_h;
    mk::lathe(o.m(Mat::Trim), t0, up, &[Vec2::new(0.0, 0.0), Vec2::new(r * 1.05, 0.0), Vec2::new(r * 1.05, 0.12), Vec2::new(r, 0.15), Vec2::new(r, 3.2), Vec2::new(r * 1.06, 3.25), Vec2::new(r * 0.08, 4.5), Vec2::new(0.0, 4.6)], 20, wood);
    // Steel hoops.
    for h in [0.5, 1.2, 1.9, 2.6] {
        mk::lathe(o.m(Mat::Metal), t0 + up * h, up, &[Vec2::new(r + 0.005, 0.0), Vec2::new(r + 0.03, 0.02), Vec2::new(r + 0.03, 0.07), Vec2::new(r + 0.005, 0.09)], 20, steel);
    }
}

/// Black plastic water tank (Latin America, South Asia, Africa).
pub fn water_tank_poly(o: &mut Out, base: Vec3, up: Vec3) {
    let c = rgb(0x1f2124);
    mk::lathe(o.m(Mat::Trim), base, up, &[
        Vec2::new(0.0, 0.0), Vec2::new(0.55, 0.0), Vec2::new(0.6, 0.08), Vec2::new(0.6, 0.3), Vec2::new(0.57, 0.33),
        Vec2::new(0.6, 0.36), Vec2::new(0.6, 0.7), Vec2::new(0.57, 0.73), Vec2::new(0.6, 0.76), Vec2::new(0.6, 1.0),
        Vec2::new(0.5, 1.15), Vec2::new(0.22, 1.22), Vec2::new(0.22, 1.3), Vec2::new(0.0, 1.32),
    ], 18, c);
}

/// Rooftop HVAC unit with a fan.
pub fn hvac(o: &mut Out, b: &Basis, size: f32) {
    let body = rgb(0xb8bcc0);
    blk(o, Mat::Trim, b, v(-size * 0.6, 0.0, -size * 0.45), v(size * 0.6, size * 0.7, size * 0.45), 0.04, body);
    // Louvered sides.
    for k in 0..6 {
        let y = 0.1 + k as f32 * size * 0.09;
        blk(o, Mat::Trim, b, v(-size * 0.55, y, size * 0.45), v(size * 0.55, y + 0.03, size * 0.47), 0.0, scale_color(body, 0.7));
    }
    let c = b.p(0.0, size * 0.7 + 0.005, 0.0);
    mk::lathe(o.m(Mat::Metal), c, b.y, &[Vec2::new(size * 0.3, 0.0), Vec2::new(size * 0.33, 0.0), Vec2::new(size * 0.33, 0.12), Vec2::new(size * 0.3, 0.12)], 20, rgb(0x5a5f66));
    mk::lathe(o.m(Mat::Metal), c, b.y, &[Vec2::new(0.0, 0.02), Vec2::new(size * 0.3, 0.02)], 20, rgb(0x2a2c30));
}

/// Stair bulkhead: small roofed box with a door.
pub fn bulkhead(o: &mut Out, b: &Basis, wall: [f32; 4], wall_mat: WallMat) {
    let (w, h, d) = (2.6, 2.6, 2.2);
    let m = o.m(Mat::Wall(wall_mat));
    mk::block(m, b, v(-w * 0.5, 0.0, -d * 0.5), v(w * 0.5, h, d * 0.5), 0.02, wall);
    blk(o, Mat::Trim, b, v(-w * 0.5 - 0.08, h, -d * 0.5 - 0.08), v(w * 0.5 + 0.08, h + 0.15, d * 0.5 + 0.08), 0.02, rgb(0x8a8680));
    let dop = Opening { x0: -0.45, y0: 0.0, w: 0.9, h: 2.05, arch: mk::Arch::Flat };
    door(o, &b.at(0.0, 0.0, d * 0.5 + 0.21), &dop, rgb(0x5a5f66), rgb(0x8a8680));
}

/// Solar panel array, tilted, on rails.
pub fn solar_array(o: &mut Out, b: &Basis, cols: usize, rows: usize, tilt: f32) {
    let (pw, ph) = (1.0, 1.65);
    let panel = rgb(0x1c2a44);
    for r in 0..rows {
        for c in 0..cols {
            let x = c as f32 * (pw + 0.04);
            let z = r as f32 * (ph * tilt.cos() + 0.6);
            let bb = Basis::new(b.p(x, 0.25, z), b.x, b.y * tilt.cos() - b.z * tilt.sin());
            mk::block(o.m(Mat::Glass), &bb, v(0.0, 0.0, -0.02), v(pw, ph, 0.02), 0.01, panel);
            // Cell grid lines.
            for k in 1..6 {
                let y = ph * k as f32 / 6.0;
                mk::block(o.m(Mat::Trim), &bb, v(0.02, y - 0.006, 0.02), v(pw - 0.02, y + 0.006, 0.025), 0.0, rgb(0xb8c0cc));
            }
            bar(o, Mat::Metal, b.p(x + pw * 0.5, 0.0, z + 0.2), bb.p(pw * 0.5, ph * 0.9, -0.02), 0.04, b.y, rgb(0x8a8e94));
        }
    }
}

/// Rooftop garden: planter beds with plants and a little pergola.
pub fn rooftop_garden(o: &mut Out, b: &Basis, w: f32, d: f32, seed: u64) {
    let wood = rgb(0x8a6a4a);
    let greens = [0x4f8a3a, 0x6aa84f, 0x3f7a3a, 0x7fa65a];
    let n = ((w / 1.6) as usize).max(1);
    for i in 0..n {
        let x0 = -w * 0.5 + i as f32 * w / n as f32 + 0.1;
        let x1 = x0 + w / n as f32 - 0.2;
        blk(o, Mat::Trim, b, v(x0, 0.0, -d * 0.5), v(x1, 0.45, -d * 0.5 + 0.8), 0.02, wood);
        let k = ((x1 - x0) / 0.45) as usize;
        for j in 0..k.max(1) {
            let c = rgb(greens[(crate::hash01(seed, (i * 13 + j) as u64) * 4.0) as usize % 4]);
            bush(o, b.p(x0 + 0.25 + j as f32 * 0.45, 0.42, -d * 0.5 + 0.4), 0.28, 0.4 + 0.4 * crate::hash01(seed, j as u64 + 9), c, seed ^ (i * 7 + j) as u64);
        }
    }
    // Pergola.
    if w > 4.0 && d > 4.0 {
        for (x, z) in [(-1.2, 0.4), (1.2, 0.4), (-1.2, 2.4), (1.2, 2.4)] {
            bar(o, Mat::Trim, b.p(x, 0.0, z), b.p(x, 2.4, z), 0.1, b.z, wood);
        }
        for k in 0..7 {
            let z = 0.2 + k as f32 * 0.38;
            bar(o, Mat::Trim, b.p(-1.4, 2.45, z), b.p(1.4, 2.45, z), 0.06, b.y, wood);
        }
    }
}

/// Brick chimney stack with a stone cap and clay pots.
pub fn chimney(o: &mut Out, base: Vec3, up: Vec3, x_axis: Vec3, height: f32, pots: usize, brick: [f32; 4]) {
    let b = Basis::new(base, x_axis, up);
    let w = 0.5 + 0.35 * pots as f32;
    mk::block(o.m(Mat::Wall(WallMat::Brick)), &b, v(-w * 0.5, -0.5, -0.35), v(w * 0.5, height, 0.35), 0.01, brick);
    blk(o, Mat::Trim, &b, v(-w * 0.5 - 0.08, height, -0.43), v(w * 0.5 + 0.08, height + 0.14, 0.43), 0.02, rgb(0xb8b0a2));
    for p in 0..pots {
        let x = -w * 0.5 + 0.42 + p as f32 * 0.35;
        mk::lathe(o.m(Mat::Trim), b.p(x, height + 0.14, 0.0), up, &[Vec2::new(0.13, 0.0), Vec2::new(0.11, 0.12), Vec2::new(0.1, 0.4), Vec2::new(0.13, 0.45), Vec2::new(0.13, 0.5), Vec2::new(0.08, 0.5), Vec2::new(0.08, 0.3)], 10, rgb(0xb5532c));
    }
}

// ---------------------------------------------------------------- lots

/// Covered porch at the front door: deck, turned columns, a shed roof.
pub fn porch(o: &mut Out, b: &Basis, x_center: f32, width: f32, depth: f32, h: f32, trim: [f32; 4], roof: [f32; 4], roof_mat: RoofMat) {
    let (xa, xb) = (x_center - width * 0.5, x_center + width * 0.5);
    blk(o, Mat::Paving, b, v(xa, -0.05, 0.0), v(xb, 0.35, depth), 0.02, rgb(0xb5aa98));
    // Steps.
    for k in 0..2 {
        blk(o, Mat::Paving, b, v(x_center - 0.7, -0.1, depth + k as f32 * 0.3), v(x_center + 0.7, 0.35 - (k + 1) as f32 * 0.12, depth + (k + 1) as f32 * 0.3), 0.015, rgb(0xb5aa98));
    }
    let col = [Vec2::new(0.14, 0.0), Vec2::new(0.14, 0.12), Vec2::new(0.1, 0.18), Vec2::new(0.09, h - 0.35), Vec2::new(0.12, h - 0.25), Vec2::new(0.14, h - 0.12), Vec2::new(0.14, h), Vec2::new(0.0, h)];
    let n = ((width / 2.2) as usize).max(1) + 1;
    for i in 0..n {
        let x = xa + 0.2 + i as f32 * (width - 0.4) / (n - 1) as f32;
        mk::lathe(o.m(Mat::Trim), b.p(x, 0.35, depth - 0.2), b.y, &col, 12, trim);
    }
    // Beam + shed roof sloping away from the wall.
    blk(o, Mat::Trim, b, v(xa, 0.35 + h - 0.05, depth - 0.32), v(xb, 0.35 + h + 0.2, depth - 0.08), 0.02, trim);
    let y_wall = 0.35 + h + 0.7;
    let y_edge = 0.35 + h + 0.2;
    let p = [b.p(xa - 0.2, y_edge, depth + 0.3), b.p(xb + 0.2, y_edge, depth + 0.3), b.p(xb + 0.2, y_wall, 0.0), b.p(xa - 0.2, y_wall, 0.0)];
    let along = b.x;
    let slope = (p[3] - p[0]).normalize();
    mk::quad_m(o.m(Mat::Roof(roof_mat)), p, along, slope, 1.0, roof);
    mk::quad_m(o.m(Mat::Trim), [p[1], p[0], p[3], p[2]], along, slope, 1.0, scale_color(trim, 0.85));
    let edge = [b.p(xa - 0.2, y_edge - 0.18, depth + 0.3), b.p(xb + 0.2, y_edge - 0.18, depth + 0.3)];
    mk::sweep(o.m(Mat::Trim), &profiles::fascia(0.05, 0.18), &edge, false, b.y, trim);
}

/// Curbside mailbox on a post.
pub fn mailbox(o: &mut Out, pos: Vec3, up: Vec3, facing: Vec3, c: [f32; 4]) {
    let side = facing.cross(up).normalize_or(Vec3::X);
    let b = Basis::new(pos, side, up);
    blk(o, Mat::Trim, &b, v(-0.05, 0.0, -0.05), v(0.05, 1.0, 0.05), 0.01, rgb(0x6b4a2f));
    // Rounded-top box: sweep a D-profile along the facing axis.
    let prof: Vec<Vec2> = [Vec2::new(-0.12, 0.0), Vec2::new(0.12, 0.0)]
        .into_iter()
        .chain((0..=8).map(|i| {
            let a = i as f32 / 8.0 * std::f32::consts::PI;
            Vec2::new(0.12 * a.cos(), 0.16 + 0.12 * a.sin())
        }))
        .chain([Vec2::new(-0.12, 0.0)])
        .collect();
    mk::sweep(o.m(Mat::Trim), &prof, &[b.p(0.0, 1.0, -0.25), b.p(0.0, 1.0, 0.25)], false, up, c);
    blk(o, Mat::Trim, &b, v(0.12, 1.1, -0.05), v(0.14, 1.32, -0.01), 0.0, rgb(0xc1121f));
}

/// Picket fence along a polyline.
pub fn picket_fence(o: &mut Out, pts: &[Vec3], up: Vec3, c: [f32; 4]) {
    let m = o.m(Mat::Trim);
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let d = (b - a).normalize_or(Vec3::X);
        let n = d.cross(up).normalize_or(Vec3::Z);
        let len = a.distance(b);
        // Rails.
        for h in [0.3, 0.8] {
            let p = [a + up * h, b + up * h, b + up * (h + 0.08), a + up * (h + 0.08)];
            m_quad2(m, p, n, c);
        }
        let k = (len / 0.14) as usize;
        for i in 0..k {
            let p0 = a + d * (i as f32 * 0.14);
            let p1 = p0 + d * 0.09;
            let q = [p0 + n * 0.03, p1 + n * 0.03, p1 + up * 1.0 + n * 0.03, p0 + up * 1.0 + n * 0.03];
            m_quad2(m, q, n, c);
            let tip = (p0 + p1) * 0.5 + up * 1.1 + n * 0.03;
            m.triangle([q[3], q[2], tip], [[0.0; 2]; 3], c);
            m.triangle([q[2], q[3], tip], [[0.0; 2]; 3], scale_color(c, 0.8));
        }
        // Posts every ~2.4 m.
        let posts = (len / 2.4).ceil() as usize;
        for i in 0..=posts {
            let p = a.lerp(b, i as f32 / posts.max(1) as f32);
            let bb = Basis::new(p, d, up);
            mk::block(m, &bb, v(-0.05, 0.0, -0.05), v(0.05, 1.15, 0.05), 0.01, c);
        }
    }
}

fn m_quad2(m: &mut MeshData, p: [Vec3; 4], n: Vec3, c: [f32; 4]) {
    let face = (p[1] - p[0]).cross(p[2] - p[0]);
    if face.dot(n) >= 0.0 {
        m.quad(p, [[0.0; 2]; 4], c);
        m.quad([p[1], p[0], p[3], p[2]], [[0.0; 2]; 4], scale_color(c, 0.85));
    } else {
        m.quad([p[1], p[0], p[3], p[2]], [[0.0; 2]; 4], c);
        m.quad(p, [[0.0; 2]; 4], scale_color(c, 0.85));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archkit::variants;
    use crate::style::Region;

    #[test]
    fn parts_build_without_panicking_and_stay_reasonable() {
        let b = Basis::new(Vec3::ZERO, Vec3::X, Vec3::Y);
        let mut o = Out::default();
        for r in Region::ALL {
            for vnt in variants(r) {
                let op = Opening { x0: 1.0, y0: 1.0, w: vnt.window.w, h: vnt.window.h, arch: vnt.window.arch };
                window(&mut o, &b, &op, &vnt.window, [1.0; 4], Some([0.3, 0.5, 0.3, 1.0]), vnt.wall_mat);
            }
        }
        door(&mut o, &b, &Opening { x0: 0.0, y0: 0.0, w: 1.0, h: 2.3, arch: mk::Arch::Round }, [0.5; 4], [1.0; 4]);
        stoop(&mut o, &b, 1.0, 0.0, 1.4, 1.6, [0.6; 4]);
        balcony(&mut o, &b, Balcony::Iron, 0.0, 2.0, 3.0, [0.1; 4], [0.8; 4], 1);
        fire_escape(&mut o, &b, 0.0, 4.0, &[4.0, 7.0, 10.0]);
        water_tank_nyc(&mut o, Vec3::ZERO, Vec3::Y);
        satellite_dish(&mut o, Vec3::ZERO, Vec3::X, Vec3::Y, 0.4);
        chimney(&mut o, Vec3::ZERO, Vec3::Y, Vec3::X, 1.2, 2, [0.6, 0.3, 0.2, 1.0]);
        porch(&mut o, &b, 2.0, 4.0, 2.0, 2.6, [1.0; 4], [0.3; 4], RoofMat::Shingle);
        picket_fence(&mut o, &[Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)], Vec3::Y, [1.0; 4]);
        let t = o.triangle_count();
        assert!(t > 2_000 && t < 200_000, "{t}");
        for m in o.meshes.values() {
            assert!(m.positions.iter().all(|p| p.iter().all(|c| c.is_finite())));
        }
    }
}
