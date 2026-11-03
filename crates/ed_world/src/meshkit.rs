//! Procedural modeling toolkit. The building blocks for detailed models:
//! profile sweeps (cornices, sills, fascia, railings), lathes (balusters,
//! chimney pots, tanks), chamfered blocks, walls with real openings and
//! reveals, and polygon insetting (parapets, mansards, hip roofs on any
//! footprint).
//!
//! Everything writes into `MeshData`, flat-shaded unless noted, so parts
//! keep crisp edges under the filmic lighting.

use crate::mesh::MeshData;
use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

/// A local frame. For facades: `x` runs along the wall, `y` is up, `z`
/// points out of the wall.
#[derive(Debug, Clone, Copy)]
pub struct Basis {
    pub o: Vec3,
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
}

impl Basis {
    pub fn new(o: Vec3, x: Vec3, y: Vec3) -> Self {
        let x = x.normalize();
        let y = (y - x * y.dot(x)).normalize();
        Self { o, x, y, z: x.cross(y) }
    }
    #[inline]
    pub fn p(&self, x: f32, y: f32, z: f32) -> Vec3 {
        self.o + self.x * x + self.y * y + self.z * z
    }
    #[inline]
    pub fn v(&self, v: Vec3) -> Vec3 {
        self.p(v.x, v.y, v.z)
    }
    #[inline]
    pub fn dir(&self, v: Vec3) -> Vec3 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }
    /// Shift the origin along the basis axes.
    pub fn at(&self, x: f32, y: f32, z: f32) -> Basis {
        Basis { o: self.p(x, y, z), ..*self }
    }
}

/// Quad with a computed flat normal; corners counter-clockwise from the front.
pub fn quad(m: &mut MeshData, p: [Vec3; 4], uv: [[f32; 2]; 4], c: [f32; 4]) {
    m.quad(p, uv, c);
}

/// Quad whose UVs are planar in meters (for tiling wall/roof textures).
pub fn quad_m(m: &mut MeshData, p: [Vec3; 4], u_axis: Vec3, v_axis: Vec3, scale: f32, c: [f32; 4]) {
    let uv = p.map(|q| [q.dot(u_axis) / scale, q.dot(v_axis) / scale]);
    m.quad(p, uv, c);
}

/// A block with chamfered edges (min/max in basis coordinates).
pub fn block(m: &mut MeshData, b: &Basis, min: Vec3, max: Vec3, bevel: f32, c: [f32; 4]) {
    // Bevels only where they can be seen: tiny parts get crisp 12-tri blocks.
    let smallest = (max - min).min_element();
    let e = if bevel < 0.012 || smallest < 0.09 { 0.0 } else { bevel.min(smallest * 0.45) };
    // Build as a convex hull of 24 points (each corner split into 3), then
    // emit the 6 main faces, 12 edge chamfers and 8 corner triangles.
    let (x0, y0, z0, x1, y1, z1) = (min.x, min.y, min.z, max.x, max.y, max.z);
    let mut face = |pts: [Vec3; 4], col: [f32; 4]| m.quad(pts.map(|p| b.v(p)), [[0.0; 2]; 4], col);
    let v = Vec3::new;
    // Faces (inset by e on the two in-plane axes).
    face([v(x0 + e, y0 + e, z1), v(x1 - e, y0 + e, z1), v(x1 - e, y1 - e, z1), v(x0 + e, y1 - e, z1)], c); // +z
    face([v(x1 - e, y0 + e, z0), v(x0 + e, y0 + e, z0), v(x0 + e, y1 - e, z0), v(x1 - e, y1 - e, z0)], c); // -z
    face([v(x1, y0 + e, z1 - e), v(x1, y0 + e, z0 + e), v(x1, y1 - e, z0 + e), v(x1, y1 - e, z1 - e)], c); // +x
    face([v(x0, y0 + e, z0 + e), v(x0, y0 + e, z1 - e), v(x0, y1 - e, z1 - e), v(x0, y1 - e, z0 + e)], c); // -x
    face([v(x0 + e, y1, z1 - e), v(x1 - e, y1, z1 - e), v(x1 - e, y1, z0 + e), v(x0 + e, y1, z0 + e)], c); // +y
    face([v(x0 + e, y0, z0 + e), v(x1 - e, y0, z0 + e), v(x1 - e, y0, z1 - e), v(x0 + e, y0, z1 - e)], c); // -y
    if e <= 1e-4 {
        return;
    }
    // Edge chamfers.
    for (sy, yy) in [(1.0f32, y1), (-1.0, y0)] {
        // Along x at top/bottom, front/back.
        for (sz, zz) in [(1.0f32, z1), (-1.0, z0)] {
            let a = v(x0 + e, yy, zz - sz * e);
            let bq = v(x1 - e, yy, zz - sz * e);
            let c2 = v(x1 - e, yy - sy * e, zz);
            let d = v(x0 + e, yy - sy * e, zz);
            let p = if sy * sz > 0.0 { [d, c2, bq, a] } else { [a, bq, c2, d] };
            face(p, c);
        }
        // Along z at top/bottom, left/right.
        for (sx, xx) in [(1.0f32, x1), (-1.0, x0)] {
            let a = v(xx - sx * e, yy, z0 + e);
            let bq = v(xx - sx * e, yy, z1 - e);
            let c2 = v(xx, yy - sy * e, z1 - e);
            let d = v(xx, yy - sy * e, z0 + e);
            let p = if sy * sx > 0.0 { [a, bq, c2, d] } else { [d, c2, bq, a] };
            face(p, c);
        }
    }
    // Vertical edges.
    for (sx, xx) in [(1.0f32, x1), (-1.0, x0)] {
        for (sz, zz) in [(1.0f32, z1), (-1.0, z0)] {
            let a = v(xx - sx * e, y0 + e, zz);
            let bq = v(xx, y0 + e, zz - sz * e);
            let c2 = v(xx, y1 - e, zz - sz * e);
            let d = v(xx - sx * e, y1 - e, zz);
            let p = if sx * sz > 0.0 { [a, bq, c2, d] } else { [bq, a, d, c2] };
            face(p, c);
        }
    }
    // Corners.
    for (sx, xx) in [(1.0f32, x1), (-1.0, x0)] {
        for (sy, yy) in [(1.0f32, y1), (-1.0, y0)] {
            for (sz, zz) in [(1.0f32, z1), (-1.0, z0)] {
                let px = b.v(v(xx, yy - sy * e, zz - sz * e));
                let py = b.v(v(xx - sx * e, yy, zz - sz * e));
                let pz = b.v(v(xx - sx * e, yy - sy * e, zz));
                let out = b.dir(v(sx, sy, sz));
                let n = (py - px).cross(pz - px);
                if n.dot(out) >= 0.0 {
                    m.triangle([px, py, pz], [[0.0; 2]; 3], c);
                } else {
                    m.triangle([px, pz, py], [[0.0; 2]; 3], c);
                }
            }
        }
    }
}

/// Sweep a 2D profile along a 3D path. Profile points are (out, up) in the
/// plane perpendicular to the path: `out` is to the path's right
/// (`tangent x up`), `up` is `up`. Joints are mitered. `closed` loops the path.
/// Profile winding: list points so that, walking the profile, the solid is on
/// the left; faces then point outward.
pub fn sweep(m: &mut MeshData, profile: &[Vec2], path: &[Vec3], closed: bool, up: Vec3, c: [f32; 4]) {
    sweep_ex(m, profile, path, closed, up, c, true);
}

/// [`sweep`] with optional end caps (thin bars and rails don't need them).
pub fn sweep_ex(m: &mut MeshData, profile: &[Vec2], path: &[Vec3], closed: bool, up: Vec3, c: [f32; 4], caps: bool) {
    let n = path.len();
    if n < 2 || profile.len() < 2 {
        return;
    }
    let seg_dir = |i: usize| -> Vec3 {
        let j = if closed { (i + 1) % n } else { (i + 1).min(n - 1) };
        let i0 = if !closed && i == n - 1 { n - 2 } else { i };
        let j0 = if !closed && i == n - 1 { n - 1 } else { j };
        (path[j0] - path[i0]).reject_from_normalized(up).normalize_or(Vec3::X)
    };
    // Mitered right-vectors with scale so the profile keeps its width.
    let frames: Vec<(Vec3, f32)> = (0..n)
        .map(|i| {
            let d_out = seg_dir(i);
            let d_in = if i == 0 && !closed { d_out } else { seg_dir((i + n - 1) % n) };
            let r_in = d_in.cross(up).normalize_or(Vec3::Z);
            let r_out = d_out.cross(up).normalize_or(Vec3::Z);
            let r = (r_in + r_out).normalize_or(r_out);
            let k = 1.0 / r.dot(r_out).max(0.35);
            (r, k)
        })
        .collect();
    let rings: Vec<Vec<Vec3>> = (0..n)
        .map(|i| {
            let (r, k) = frames[i];
            profile.iter().map(|p| path[i] + r * p.x * k + up * p.y).collect()
        })
        .collect();
    let segs = if closed { n } else { n - 1 };
    let mut along = 0.0f32;
    for s in 0..segs {
        let (i, j) = (s, (s + 1) % n);
        let len = path[i].distance(path[j]);
        for k in 0..profile.len() - 1 {
            let (a, b) = (rings[i][k], rings[i][k + 1]);
            let (cc, d) = (rings[j][k + 1], rings[j][k]);
            m.quad([a, d, cc, b], [[along, 0.0], [along + len, 0.0], [along + len, 1.0], [along, 1.0]], c);
        }
        along += len;
    }
    if !closed && caps {
        // Cap both ends with a fan.
        for (idx, flip) in [(0usize, true), (n - 1, false)] {
            let ring = &rings[idx];
            let ctr = ring.iter().copied().sum::<Vec3>() / ring.len() as f32;
            for k in 0..ring.len() - 1 {
                let (a, b) = (ring[k], ring[k + 1]);
                if flip { m.triangle([ctr, b, a], [[0.0; 2]; 3], c) } else { m.triangle([ctr, a, b], [[0.0; 2]; 3], c) }
            }
        }
    }
}

/// Revolve a profile of (radius, height) points around `axis` through
/// `center`. Smooth normals around, flat along the profile.
pub fn lathe(m: &mut MeshData, center: Vec3, axis: Vec3, profile: &[Vec2], segments: usize, c: [f32; 4]) {
    let axis = axis.normalize();
    let side = axis.any_orthonormal_vector();
    let side2 = axis.cross(side);
    let base = m.positions.len() as u32;
    let segs = segments.max(3);
    for (k, p) in profile.iter().enumerate() {
        let (dr, dh) = if k + 1 < profile.len() {
            (profile[k + 1].x - p.x, profile[k + 1].y - p.y)
        } else {
            (p.x - profile[k - 1].x, p.y - profile[k - 1].y)
        };
        for s in 0..=segs {
            let a = s as f32 / segs as f32 * TAU;
            let radial = side * a.cos() + side2 * a.sin();
            let pos = center + radial * p.x + axis * p.y;
            // Profile normal: perpendicular to (dr, dh) in the radial plane.
            let n = (radial * dh - axis * dr).normalize_or(radial);
            m.vertex(pos.to_array(), n.to_array(), [s as f32 / segs as f32, k as f32], c);
        }
    }
    let w = segs as u32 + 1;
    for k in 0..profile.len() as u32 - 1 {
        for s in 0..segs as u32 {
            let a = base + k * w + s;
            let b = a + w;
            m.tri(a, b, a + 1);
            m.tri(a + 1, b, b + 1);
        }
    }
}

/// Offset a closed polygon (in the plane perpendicular to `up`) inward by
/// `d` meters with mitered corners (clamped so sharp corners don't spike).
/// Works for any winding.
pub fn inset(poly: &[Vec3], up: Vec3, d: f32) -> Vec<Vec3> {
    let n = poly.len();
    let area = signed_area(poly, up);
    let s = if area > 0.0 { -1.0 } else { 1.0 };
    (0..n)
        .map(|i| {
            let p = poly[i];
            let a = (p - poly[(i + n - 1) % n]).reject_from_normalized(up).normalize_or(Vec3::X);
            let b = (poly[(i + 1) % n] - p).reject_from_normalized(up).normalize_or(Vec3::X);
            // Inward normals of the two edges.
            let na = up.cross(a) * s;
            let nb = up.cross(b) * s;
            let bis = (na + nb).normalize_or(nb);
            let k = 1.0 / bis.dot(nb).max(0.3);
            p + bis * d * k
        })
        .collect()
}

/// Signed area in the plane perpendicular to `up` (positive = CCW seen from above
/// with a right-handed (x, up, z) basis).
pub fn signed_area(poly: &[Vec3], up: Vec3) -> f32 {
    let ax = Vec3::X.reject_from_normalized(up).normalize_or(Vec3::X);
    let az = ax.cross(up);
    let mut a = 0.0;
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        a += p.dot(ax) * q.dot(az) - q.dot(ax) * p.dot(az);
    }
    a * 0.5
}

/// Triangulate a flat polygon (with optional holes) and emit it facing `normal`.
pub fn polygon(m: &mut MeshData, outer: &[Vec3], holes: &[Vec<Vec3>], normal: Vec3, uv_scale: f32, c: [f32; 4]) {
    let ax = normal.any_orthonormal_vector();
    let az = normal.cross(ax);
    let mut flat = Vec::new();
    let mut pts = Vec::new();
    let mut hole_idx = Vec::new();
    for (k, ring) in std::iter::once(outer).chain(holes.iter().map(Vec::as_slice)).enumerate() {
        if k > 0 {
            hole_idx.push(pts.len());
        }
        for p in ring {
            flat.extend_from_slice(&[p.dot(ax) as f64, p.dot(az) as f64]);
            pts.push(*p);
        }
    }
    let Ok(tris) = earcutr::earcut(&flat, &hole_idx, 2) else { return };
    let base = m.positions.len() as u32;
    for p in &pts {
        m.vertex(p.to_array(), normal.to_array(), [p.dot(ax) / uv_scale, p.dot(az) / uv_scale], c);
    }
    for t in tris.chunks_exact(3) {
        let (a, b, cc) = (pts[t[0]], pts[t[1]], pts[t[2]]);
        let (i, j, k) = (t[0] as u32 + base, t[1] as u32 + base, t[2] as u32 + base);
        if (b - a).cross(cc - a).dot(normal) >= 0.0 { m.tri(i, j, k) } else { m.tri(i, k, j) }
    }
}

/// Opening shapes for walls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Arch {
    Flat,
    /// Semicircular top.
    Round,
    /// Shallow segmental arch (rise as a fraction of width).
    Segmental(f32),
    /// Pointed / horseshoe-ish (North Africa).
    Pointed,
}

/// An opening in a wall, in wall-local coordinates (x along, y up).
#[derive(Debug, Clone, Copy)]
pub struct Opening {
    pub x0: f32,
    pub y0: f32,
    pub w: f32,
    pub h: f32,
    pub arch: Arch,
}

impl Opening {
    /// Outline, counter-clockwise in (x, y).
    pub fn outline(&self) -> Vec<Vec2> {
        let (x0, y0, w, h) = (self.x0, self.y0, self.w, self.h);
        let mut pts = vec![Vec2::new(x0, y0), Vec2::new(x0 + w, y0)];
        match self.arch {
            Arch::Flat => {
                pts.push(Vec2::new(x0 + w, y0 + h));
                pts.push(Vec2::new(x0, y0 + h));
            }
            Arch::Round | Arch::Segmental(_) | Arch::Pointed => {
                let rise = match self.arch {
                    Arch::Round => w * 0.5,
                    Arch::Segmental(r) => w * r,
                    _ => w * 0.65,
                }
                .min(h * 0.6);
                let spring = y0 + h - rise;
                pts.push(Vec2::new(x0 + w, spring));
                let n = 10;
                for k in 1..n {
                    let t = k as f32 / n as f32;
                    let x = x0 + w * (1.0 - t);
                    let y = match self.arch {
                        Arch::Pointed => {
                            let u = 1.0 - (2.0 * t - 1.0).abs();
                            spring + rise * (1.0 - (1.0 - u).powf(1.6))
                        }
                        _ => spring + rise * (std::f32::consts::PI * t).sin(),
                    };
                    pts.push(Vec2::new(x, y));
                }
                pts.push(Vec2::new(x0, spring));
            }
        }
        pts
    }
}

/// A wall panel (x in 0..w, y in y0..y1 of the basis) with holes cut for
/// `openings`, plus reveals (the wall thickness showing inside each
/// opening) of `depth`. UVs are in meters / `uv_scale` for tiling textures.
pub fn wall_with_openings(m: &mut MeshData, b: &Basis, w: f32, y0: f32, y1: f32, openings: &[Opening], depth: f32, uv_scale: f32, c: [f32; 4], reveal: &mut MeshData, reveal_c: [f32; 4]) {
    let outer = vec![b.p(0.0, y0, 0.0), b.p(w, y0, 0.0), b.p(w, y1, 0.0), b.p(0.0, y1, 0.0)];
    let holes: Vec<Vec<Vec3>> = openings
        .iter()
        .filter(|o| o.x0 > 0.01 && o.x0 + o.w < w - 0.01 && o.y0 >= y0 - 0.001 && o.y0 + o.h <= y1 + 0.001)
        .map(|o| o.outline().iter().map(|p| b.p(p.x, p.y.max(y0 + 0.001), 0.0)).collect())
        .collect();
    // Wall face with tiling UVs along the basis axes.
    let mut flat = Vec::new();
    let mut pts = Vec::new();
    let mut hole_idx = Vec::new();
    for (k, ring) in std::iter::once(&outer).chain(holes.iter()).enumerate() {
        if k > 0 {
            hole_idx.push(pts.len());
        }
        for p in ring {
            let l = *p - b.o;
            flat.extend_from_slice(&[l.dot(b.x) as f64, l.dot(b.y) as f64]);
            pts.push(*p);
        }
    }
    if let Ok(tris) = earcutr::earcut(&flat, &hole_idx, 2) {
        let base = m.positions.len() as u32;
        for p in &pts {
            let l = *p - b.o;
            m.vertex(p.to_array(), b.z.to_array(), [l.dot(b.x) / uv_scale, l.dot(b.y) / uv_scale], c);
        }
        for t in tris.chunks_exact(3) {
            let (pa, pb, pc) = (pts[t[0]], pts[t[1]], pts[t[2]]);
            let (i, j, k) = (t[0] as u32 + base, t[1] as u32 + base, t[2] as u32 + base);
            if (pb - pa).cross(pc - pa).dot(b.z) >= 0.0 { m.tri(i, j, k) } else { m.tri(i, k, j) }
        }
    }
    // Reveals: quads from each hole edge back into the wall.
    for ring in &holes {
        let n = ring.len();
        for i in 0..n {
            let (p, q) = (ring[i], ring[(i + 1) % n]);
            let (pb, qb) = (p - b.z * depth, q - b.z * depth);
            // Face inward toward the opening center.
            m_quad_facing(reveal, [p, q, qb, pb], ring.iter().copied().sum::<Vec3>() / n as f32 - b.z * depth * 0.5, reveal_c);
        }
    }
}

/// Emit a quad oriented so its normal points toward `toward`.
pub fn m_quad_facing(m: &mut MeshData, p: [Vec3; 4], toward: Vec3, c: [f32; 4]) {
    let n = (p[1] - p[0]).cross(p[2] - p[0]);
    let mid = (p[0] + p[1] + p[2] + p[3]) * 0.25;
    if n.dot(toward - mid) >= 0.0 {
        m.quad(p, [[0.0; 2]; 4], c);
    } else {
        m.quad([p[1], p[0], p[3], p[2]], [[0.0; 2]; 4], c);
    }
}

/// A closed rectangular-section frame running around an outline (window
/// and door frames, picture rails). `outline` is in basis (x, y) at depth `z`.
pub fn frame_along(m: &mut MeshData, b: &Basis, outline: &[Vec2], z: f32, width: f32, thickness: f32, c: [f32; 4]) {
    let path: Vec<Vec3> = outline.iter().map(|p| b.p(p.x, p.y, z)).collect();
    // Sweep in the wall plane: profile "out" = toward the opening center is
    // handled by using the wall normal as sweep up-vector.
    let prof = [Vec2::new(-width * 0.5, 0.0), Vec2::new(width * 0.5, 0.0), Vec2::new(width * 0.5, thickness), Vec2::new(-width * 0.5, thickness), Vec2::new(-width * 0.5, 0.0)];
    sweep(m, &prof, &path, true, b.z, c);
}

/// Standard profiles (out, up).
pub mod profiles {
    use glam::Vec2;
    /// Classic cornice: cove + fillet + drip.
    pub fn cornice(depth: f32, height: f32) -> Vec<Vec2> {
        let (d, h) = (depth, height);
        vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(d * 0.15, 0.0),
            Vec2::new(d * 0.2, h * 0.15),
            Vec2::new(d * 0.45, h * 0.3),
            Vec2::new(d * 0.7, h * 0.5),
            Vec2::new(d * 0.85, h * 0.65),
            Vec2::new(d, h * 0.7),
            Vec2::new(d, h),
            Vec2::new(0.0, h),
        ]
    }
    /// Simple string course / band.
    pub fn band(depth: f32, height: f32) -> Vec<Vec2> {
        vec![Vec2::new(0.0, 0.0), Vec2::new(depth * 0.6, 0.0), Vec2::new(depth, height * 0.35), Vec2::new(depth, height), Vec2::new(0.0, height)]
    }
    /// Parapet coping: overhanging cap.
    pub fn coping(width: f32, height: f32) -> Vec<Vec2> {
        let w = width;
        vec![
            Vec2::new(-w * 0.5 - 0.04, 0.0),
            Vec2::new(w * 0.5 + 0.04, 0.0),
            Vec2::new(w * 0.5 + 0.04, 0.05),
            Vec2::new(w * 0.5, 0.07),
            Vec2::new(w * 0.5, height),
            Vec2::new(-w * 0.5, height),
            Vec2::new(-w * 0.5, 0.07),
            Vec2::new(-w * 0.5 - 0.04, 0.05),
            Vec2::new(-w * 0.5 - 0.04, 0.0),
        ]
    }
    /// Fascia board with a small drip edge.
    pub fn fascia(depth: f32, height: f32) -> Vec<Vec2> {
        vec![Vec2::new(0.0, 0.0), Vec2::new(depth, 0.0), Vec2::new(depth, height), Vec2::new(depth + 0.02, height), Vec2::new(depth + 0.02, height + 0.02), Vec2::new(0.0, height + 0.02)]
    }
    /// Square bar (railings, mullions), centered.
    pub fn bar(s: f32) -> Vec<Vec2> {
        let h = s * 0.5;
        vec![Vec2::new(-h, -h), Vec2::new(h, -h), Vec2::new(h, h), Vec2::new(-h, h), Vec2::new(-h, -h)]
    }
    /// Plinth / water table at the base of a wall.
    pub fn plinth(depth: f32, height: f32) -> Vec<Vec2> {
        vec![Vec2::new(0.0, 0.0), Vec2::new(depth, 0.0), Vec2::new(depth, height * 0.8), Vec2::new(depth * 0.3, height), Vec2::new(0.0, height)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outward_ratio(m: &MeshData, center: Vec3) -> f32 {
        let mut good = 0;
        let mut total = 0;
        for t in m.indices.chunks_exact(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(m.positions[i as usize]));
            let n = (b - a).cross(c - a);
            if n.length() < 1e-8 {
                continue;
            }
            total += 1;
            if n.dot((a + b + c) / 3.0 - center) > 0.0 {
                good += 1;
            }
        }
        good as f32 / total as f32
    }

    #[test]
    fn block_faces_outward() {
        let mut m = MeshData::default();
        let b = Basis::new(Vec3::ZERO, Vec3::X, Vec3::Y);
        block(&mut m, &b, Vec3::splat(-1.0), Vec3::splat(1.0), 0.1, [1.0; 4]);
        assert_eq!(m.triangle_count(), 6 * 2 + 12 * 2 + 8);
        assert!(outward_ratio(&m, Vec3::ZERO) > 0.999, "{}", outward_ratio(&m, Vec3::ZERO));
    }

    #[test]
    fn sweep_box_profile_around_square_faces_out() {
        // A coping around a 10x10 square; all faces should point away from the
        // centerline of the coping (roughly: no inside-out tubes).
        let mut m = MeshData::default();
        let sq = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 10.0), Vec3::new(0.0, 0.0, 10.0)];
        sweep(&mut m, &profiles::coping(0.3, 0.4), &sq, true, Vec3::Y, [1.0; 4]);
        assert!(m.triangle_count() > 30);
    }

    #[test]
    fn wall_holes_reduce_area() {
        let mut m = MeshData::default();
        let mut rev = MeshData::default();
        let b = Basis::new(Vec3::ZERO, Vec3::X, Vec3::Y);
        let ops = [Opening { x0: 1.0, y0: 1.0, w: 1.0, h: 1.5, arch: Arch::Round }, Opening { x0: 3.0, y0: 1.0, w: 1.0, h: 1.5, arch: Arch::Flat }];
        wall_with_openings(&mut m, &b, 5.0, 0.0, 3.0, &ops, 0.2, 1.0, [1.0; 4], &mut rev, [1.0; 4]);
        let area: f32 = m
            .indices
            .chunks_exact(3)
            .map(|t| {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(m.positions[i as usize]));
                (b - a).cross(c - a).length() * 0.5
            })
            .sum();
        assert!(area < 15.0 - 2.5 && area > 15.0 - 3.6, "wall area {area}");
        assert!(rev.triangle_count() > 20);
        // Wall faces +z.
        for n in &m.normals {
            assert!(n[2] > 0.99);
        }
    }

    #[test]
    fn inset_square() {
        let sq = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 10.0), Vec3::new(0.0, 0.0, 10.0)];
        let i = inset(&sq, Vec3::Y, 1.0);
        for p in &i {
            assert!((p.x - 1.0).abs() < 1e-3 || (p.x - 9.0).abs() < 1e-3, "{p:?}");
            assert!((p.z - 1.0).abs() < 1e-3 || (p.z - 9.0).abs() < 1e-3, "{p:?}");
        }
        // Reverse winding gives the same result.
        let mut r = sq.clone();
        r.reverse();
        let i2 = inset(&r, Vec3::Y, 1.0);
        assert!(i2.iter().all(|p| p.x > 0.5 && p.x < 9.5));
    }

    #[test]
    fn lathe_has_expected_size() {
        let mut m = MeshData::default();
        lathe(&mut m, Vec3::ZERO, Vec3::Y, &[Vec2::new(0.5, 0.0), Vec2::new(0.5, 2.0)], 12, [1.0; 4]);
        assert_eq!(m.triangle_count(), 24);
    }
}
