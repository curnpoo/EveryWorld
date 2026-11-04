//! Engine-free geometry for the map globe: a unit sphere textured with
//! Web-Mercator satellite tiles.
//!
//! Globe space: Earth radius 1, +Y north, lon 0 on +X, 90°E on -Z (this is
//! ECEF `(x, y, z)` mapped to `(x, z, -y)`, a proper rotation). Tile
//! latitudes are the Web-Mercator "geodetic" latitudes used by the imagery,
//! so a tile's pixels land exactly where the vertices say they do.

use crate::mesh::MeshData;
use ed_geo::tile::TileId;
use glam::{Vec2, Vec3};
use std::f64::consts::PI;

/// Latitude beyond which Web-Mercator has no imagery.
pub const MAX_LAT_DEG: f64 = 85.051_128_78;

/// Unit-sphere point for a latitude/longitude in degrees.
pub fn unit_from_latlon(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new((la.cos() * lo.cos()) as f32, la.sin() as f32, (-la.cos() * lo.sin()) as f32)
}

/// Inverse of [`unit_from_latlon`]: `(lat_deg, lon_deg)`.
pub fn latlon_from_unit(p: Vec3) -> (f64, f64) {
    let p = p.normalize().as_dvec3();
    (p.y.clamp(-1.0, 1.0).asin().to_degrees(), (-p.z).atan2(p.x).to_degrees())
}

/// Latitude (degrees) at Web-Mercator world-y in 0..1 (0 = north edge).
fn lat_at_world_y(wy: f64) -> f64 {
    (PI * (1.0 - 2.0 * wy)).sinh().atan().to_degrees()
}

fn lon_at_world_x(wx: f64) -> f64 {
    wx * 360.0 - 180.0
}

/// Point on the sphere at fractional tile coords `(u, v)`.
pub fn tile_point(id: TileId, u: f64, v: f64) -> Vec3 {
    let n = (1u64 << id.z) as f64;
    unit_from_latlon(lat_at_world_y((id.y as f64 + v) / n), lon_at_world_x((id.x as f64 + u) / n))
}

/// Direction through the middle of a tile.
pub fn tile_center(id: TileId) -> Vec3 {
    tile_point(id, 0.5, 0.5)
}

/// Roughly how large a tile is on the unit sphere (radians, half-extent).
pub fn tile_angular_radius(id: TileId) -> f32 {
    let n = (1u64 << id.z) as f64;
    let lat = lat_at_world_y((id.y as f64 + 0.5) / n).to_radians();
    // East-west extent shrinks with cos(lat); north-south is the Mercator row height.
    let ew = 2.0 * PI / n * lat.cos().max(0.02);
    let ns = 2.0 * PI / n * lat.cos().max(0.02); // Mercator rows are conformal: same scale
    (0.5 * ew.hypot(ns)) as f32
}

/// Tile mesh: an `n x n` quad grid plus a short skirt that hides cracks to
/// neighbours at other levels. UVs are the tile's own `(u, v)`.
pub fn tile_mesh(id: TileId, n: usize) -> MeshData {
    let mut m = MeshData::default();
    let skirt = 0.004 * (0.5f32).powi(id.z as i32 / 2).max(0.0006);
    let mut top = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            let (u, v) = (i as f64 / n as f64, j as f64 / n as f64);
            let p = tile_point(id, u, v);
            top.push(m.vertex(p.to_array(), p.to_array(), [u as f32, v as f32], [1.0; 4]));
        }
    }
    let at = |i: usize, j: usize| top[j * (n + 1) + i];
    for j in 0..n {
        for i in 0..n {
            // Counter-clockwise seen from outside: u runs east, v runs south.
            m.tri(at(i, j), at(i, j + 1), at(i + 1, j));
            m.tri(at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
        }
    }
    // Skirts along the four edges, dropped toward the centre.
    let edges: [Vec<(usize, usize)>; 4] = [
        (0..=n).map(|i| (i, 0)).collect(),
        (0..=n).map(|i| (i, n)).collect(),
        (0..=n).map(|j| (0, j)).collect(),
        (0..=n).map(|j| (n, j)).collect(),
    ];
    for edge in edges.iter() {
        let mut lower = Vec::new();
        for &(i, j) in edge {
            let (u, v) = (i as f64 / n as f64, j as f64 / n as f64);
            let p = tile_point(id, u, v);
            lower.push(m.vertex((p * (1.0 - skirt)).to_array(), p.to_array(), [u as f32, v as f32], [1.0; 4]));
        }
        for w in 0..edge.len() - 1 {
            let (a, b) = (at(edge[w].0, edge[w].1), at(edge[w + 1].0, edge[w + 1].1));
            let (c, d) = (lower[w], lower[w + 1]);
            // Wind both ways; the skirt is a cheap hidden filler, so culling is off for it anyway.
            m.tri(a, c, b);
            m.tri(b, c, d);
            m.tri(a, b, c);
            m.tri(b, d, c);
        }
    }
    m
}

/// Fan covering everything poleward of the Mercator limit. `north` picks the
/// pole; UV.x is longitude (0..1), UV.y is 0 at the rim and 1 at the pole.
pub fn polar_cap(north: bool, segments: usize, rings: usize) -> MeshData {
    let mut m = MeshData::default();
    let sgn = if north { 1.0 } else { -1.0 };
    let mut idx = Vec::new();
    for r in 0..=rings {
        let t = r as f64 / rings as f64;
        let lat = sgn * (MAX_LAT_DEG + (90.0 - MAX_LAT_DEG) * t);
        for s in 0..=segments {
            let lon = -180.0 + 360.0 * s as f64 / segments as f64;
            let p = unit_from_latlon(lat, lon);
            idx.push(m.vertex(p.to_array(), p.to_array(), [s as f32 / segments as f32, t as f32], [1.0; 4]));
        }
    }
    let at = |s: usize, r: usize| idx[r * (segments + 1) + s];
    for r in 0..rings {
        for s in 0..segments {
            let (a, b, c, d) = (at(s, r), at(s + 1, r), at(s, r + 1), at(s + 1, r + 1));
            if north {
                m.tri(a, b, c);
                m.tri(b, d, c);
            } else {
                m.tri(a, c, b);
                m.tri(b, c, d);
            }
        }
    }
    m
}

/// Ray vs. unit sphere: nearest hit in front of the origin, if any.
pub fn ray_sphere(origin: Vec3, dir: Vec3) -> Option<Vec3> {
    let b = origin.dot(dir);
    let c = origin.length_squared() - 1.0;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    (t > 0.0).then(|| origin + dir * t)
}

/// Pick the set of tiles to draw: refine until a tile's texels are about one
/// screen pixel, skipping tiles beyond the horizon. `px_per_rad` is screen
/// pixels per radian at unit distance (`height / (2 tan(fov/2))`).
pub fn select_tiles(cam: Vec3, view_dir: Vec3, half_fov: f32, px_per_rad: f32, max_z: u8, texel_px: f32) -> Vec<TileId> {
    let mut out = Vec::new();
    let dist = cam.length().max(1.0001);
    let cam_dir = cam / dist;
    // Angle from the sub-camera point to the horizon.
    let horizon = (1.0 / dist).clamp(-1.0, 1.0).acos();
    let mut stack = vec![TileId::new(0, 0, 0)];
    while let Some(t) = stack.pop() {
        let c = tile_center(t);
        let r = tile_angular_radius(t);
        let ang = cam_dir.dot(c).clamp(-1.0, 1.0).acos();
        if ang - r * 1.2 > horizon {
            continue;
        }
        // Outside the view cone (with the tile's own size as slack)?
        let to = c - cam;
        let d = to.length().max(1e-6);
        let slack = (r * 1.2 / d).clamp(0.0, 1.5).atan();
        // A tile under the camera always matters, however far off axis its centre is.
        if ang > r * 1.25 && (to / d).dot(view_dir).clamp(-1.0, 1.0).acos() > half_fov + slack {
            continue;
        }
        if t.z >= max_z {
            out.push(t);
            continue;
        }
        // Distance from the camera to the nearest part of the tile (roughly).
        let nearest = (cam.distance(c) - r * 1.0).max(dist - 1.0).max(1e-5);
        // Apparent width of the tile in pixels (two radii across) vs 256 texels.
        let apparent = 2.0 * r * px_per_rad / nearest.max(1e-5);
        if t.z < 1 || apparent > 256.0 * texel_px {
            for ch in children(t) {
                stack.push(ch);
            }
        } else {
            out.push(t);
        }
    }
    out
}

pub fn children(t: TileId) -> [TileId; 4] {
    let (x, y, z) = (t.x * 2, t.y * 2, t.z + 1);
    [TileId::new(z, x, y), TileId::new(z, x + 1, y), TileId::new(z, x, y + 1), TileId::new(z, x + 1, y + 1)]
}

/// Screen-space heading helper: unit tangent toward north at a surface point.
pub fn north_at(p: Vec3) -> Vec3 {
    let n = Vec3::Y - p * p.y;
    if n.length_squared() < 1e-8 { Vec3::NEG_Z } else { n.normalize() }
}

/// East tangent at a surface point (right-handed with up = p, north).
pub fn east_at(p: Vec3) -> Vec3 {
    north_at(p).cross(p).normalize_or_zero()
}

/// Great-circle heading (degrees clockwise from north) -> world direction.
pub fn tangent_for_heading(p: Vec3, heading_deg: f32) -> Vec3 {
    let h = heading_deg.to_radians();
    (north_at(p) * h.cos() + east_at(p) * h.sin()).normalize_or_zero()
}

/// Convenience for UI math.
pub fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latlon_roundtrip() {
        for (la, lo) in [(0.0, 0.0), (46.02, 7.75), (-33.9, 151.2), (80.0, -120.0)] {
            let (a, b) = latlon_from_unit(unit_from_latlon(la, lo));
            assert!((a - la).abs() < 1e-4 && (b - lo).abs() < 1e-4, "{la},{lo} -> {a},{b}");
        }
    }

    #[test]
    fn east_is_east() {
        let p = unit_from_latlon(10.0, 20.0);
        let e = east_at(p);
        let q = unit_from_latlon(10.0, 20.5);
        assert!(e.dot(q - p) > 0.0);
        assert!(north_at(p).dot(unit_from_latlon(10.5, 20.0) - p) > 0.0);
    }

    #[test]
    fn root_tile_spans_the_mercator_square() {
        let t = TileId::new(0, 0, 0);
        assert!((latlon_from_unit(tile_point(t, 0.5, 0.0)).0 - MAX_LAT_DEG).abs() < 1e-3);
        assert!((latlon_from_unit(tile_point(t, 0.5, 1.0)).0 + MAX_LAT_DEG).abs() < 1e-3);
        assert!((latlon_from_unit(tile_point(t, 0.0, 0.5)).1 + 180.0).abs() < 1e-3);
    }

    #[test]
    fn tile_triangles_face_outward() {
        let m = tile_mesh(TileId::new(3, 4, 3), 4);
        let grid = 4 * 4 * 2 * 3; // surface triangles first
        for t in m.indices[..grid].chunks(3) {
            let p = |i: u32| Vec3::from(m.positions[i as usize]);
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let n = (b - a).cross(c - a);
            assert!(n.dot(a + b + c) > 0.0, "inward triangle");
        }
    }

    #[test]
    fn farther_means_fewer_tiles_and_they_cover_the_view() {
        let px = 720.0 / (2.0 * (0.4f32).tan());
        let far = select_tiles(Vec3::new(0.0, 0.0, 3.0), Vec3::NEG_Z, 0.5, px, 12, 1.0);
        let spot = unit_from_latlon(46.0, 7.7);
        let near = select_tiles(spot * 1.002, -spot, 0.5, px, 12, 1.0);
        assert!(far.len() < 200, "far {}", far.len());
        assert!(near.len() < 600, "near {}", near.len());
        assert!(near.iter().any(|t| t.z >= 9), "max z {:?} n={}", near.iter().map(|t| t.z).max(), near.len());
        assert!(far.iter().all(|t| t.z <= 6));
    }

    #[test]
    fn ray_hits_front_of_sphere() {
        let h = ray_sphere(Vec3::new(0.0, 0.0, 3.0), Vec3::NEG_Z).unwrap();
        assert!((h - Vec3::Z).length() < 1e-5);
        assert!(ray_sphere(Vec3::new(0.0, 2.0, 3.0), Vec3::NEG_Z).is_none());
    }
}
