//! Terrain tile meshes.
//!
//! A [`TerrainGrid`] is the single source of truth for ground height inside a
//! tile: the render mesh, the physics collider, and everything that sits on
//! the ground (buildings, roads, trees, water) all sample the *same* grid with
//! the *same* triangle interpolation, so nothing floats or sinks.

use crate::mesh::MeshData;
use crate::project::TileProjector;
use crate::surface::SurfaceMap;
use ed_data::elevation::HeightTile;
use ed_geo::tile::TileId;
use glam::Vec3;

#[derive(Debug, Clone)]
pub struct TerrainGrid {
    pub tile: TileId,
    /// Quads per side.
    pub segments: usize,
    /// (segments+1)^2 heights, row 0 = north.
    pub heights: Vec<f32>,
}

impl TerrainGrid {
    /// Build from an elevation tile that is this tile or one of its ancestors.
    pub fn from_heights(tile: TileId, src: &HeightTile, segments: usize) -> Self {
        let (u0, v0, s) = if src.id == tile {
            (0.0, 0.0, 1.0)
        } else {
            let (anc, u0, v0, s) = tile.ancestor(src.id.z);
            debug_assert_eq!(anc, src.id);
            (u0, v0, s)
        };
        let n = segments + 1;
        let mut heights = Vec::with_capacity(n * n);
        for j in 0..n {
            for i in 0..n {
                let u = u0 + s * i as f64 / segments as f64;
                let v = v0 + s * j as f64 / segments as f64;
                heights.push(src.sample(u, v));
            }
        }
        Self { tile, segments, heights }
    }

    pub fn flat(tile: TileId, segments: usize, h: f32) -> Self {
        Self { tile, segments, heights: vec![h; (segments + 1) * (segments + 1)] }
    }

    #[inline]
    fn h(&self, i: usize, j: usize) -> f32 {
        let n = self.segments + 1;
        self.heights[j.min(self.segments) * n + i.min(self.segments)]
    }

    /// Height at tile (u, v), interpolated on the same triangles the mesh uses.
    pub fn sample(&self, u: f64, v: f64) -> f32 {
        let s = self.segments as f64;
        let fx = (u.clamp(0.0, 1.0) * s).min(s - 1e-9);
        let fy = (v.clamp(0.0, 1.0) * s).min(s - 1e-9);
        let (i, j) = (fx.floor() as usize, fy.floor() as usize);
        let (tx, ty) = ((fx - i as f64) as f32, (fy - j as f64) as f32);
        let (h00, h10, h01, h11) = (self.h(i, j), self.h(i + 1, j), self.h(i, j + 1), self.h(i + 1, j + 1));
        // Quads are split along the 00-11 diagonal.
        if tx >= ty {
            h00 + (h10 - h00) * tx + (h11 - h10) * ty
        } else {
            h00 + (h11 - h01) * tx + (h01 - h00) * ty
        }
    }

    pub fn min_max(&self) -> (f32, f32) {
        self.heights.iter().fold((f32::MAX, f32::MIN), |(a, b), &h| (a.min(h), b.max(h)))
    }
}

/// Build the render mesh. Vertex colors come from the surface map (land
/// cover + biome + slope + snowline). `skirt_m` hangs a curtain off each edge
/// to hide cracks between neighbouring tiles at different LODs.
pub fn build_mesh(grid: &TerrainGrid, proj: &TileProjector, surface: &SurfaceMap, skirt_m: f32) -> MeshData {
    let seg = grid.segments;
    let n = seg + 1;
    let mut m = MeshData::default();
    let mut pos = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let (u, v) = (i as f64 / seg as f64, j as f64 / seg as f64);
            pos.push(proj.local(u, v, grid.h(i, j) as f64));
        }
    }
    // Smooth normals from the grid.
    let up = proj.up();
    let normal = |i: usize, j: usize| -> Vec3 {
        let l = pos[j * n + i.saturating_sub(1)];
        let r = pos[j * n + (i + 1).min(seg)];
        let t = pos[j.saturating_sub(1) * n + i];
        let b = pos[(j + 1).min(seg) * n + i];
        let nrm = (r - l).cross(t - b).normalize_or_zero();
        if nrm.dot(up) < 0.0 { -nrm } else { nrm }
    };
    for j in 0..n {
        for i in 0..n {
            let (u, v) = (i as f64 / seg as f64, j as f64 / seg as f64);
            let nrm = normal(i, j);
            let slope = 1.0 - nrm.dot(up).clamp(0.0, 1.0);
            let color = surface.ground_color(u, v, grid.h(i, j), slope);
            m.vertex(pos[j * n + i].to_array(), nrm.to_array(), [u as f32, v as f32], color);
        }
    }
    for j in 0..seg {
        for i in 0..seg {
            let a = (j * n + i) as u32;
            let b = a + 1;
            let c = a + n as u32;
            let d = c + 1;
            // Diagonal a-d, matching TerrainGrid::sample. Winding: CCW from above
            // in render space (x east, z south).
            m.tri(a, c, d);
            m.tri(a, d, b);
        }
    }
    if skirt_m > 0.0 {
        let edges: [Vec<usize>; 4] = [
            (0..n).collect(),                                // north edge
            (0..n).map(|i| seg * n + i).rev().collect(),     // south edge
            (0..n).map(|j| j * n).rev().collect(),           // west edge
            (0..n).map(|j| j * n + seg).collect(),           // east edge
        ];
        for edge in edges {
            let mut prev: Option<(u32, u32)> = None;
            for &k in &edge {
                let top = k as u32;
                let p = Vec3::from(m.positions[k]) - up * skirt_m;
                let bot = m.vertex(p.to_array(), m.normals[k], m.uvs[k], m.colors[k]);
                if let Some((pt, pb)) = prev {
                    // Double-sided so winding doesn't matter on any edge.
                    m.tri(pt, top, bot);
                    m.tri(pt, bot, pb);
                    m.tri(pt, bot, top);
                    m.tri(pt, pb, bot);
                }
                prev = Some((top, bot));
            }
        }
    }
    m
}

/// Collision geometry: the same surface minus skirts, optionally decimated.
pub fn build_collider(grid: &TerrainGrid, proj: &TileProjector) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let seg = grid.segments;
    let n = seg + 1;
    let mut verts = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            verts.push(proj.local(i as f64 / seg as f64, j as f64 / seg as f64, grid.h(i, j) as f64));
        }
    }
    let mut tris = Vec::with_capacity(seg * seg * 2);
    for j in 0..seg {
        for i in 0..seg {
            let a = (j * n + i) as u32;
            let (b, c) = (a + 1, a + n as u32);
            let d = c + 1;
            tris.push([a, c, d]);
            tris.push([a, d, b]);
        }
    }
    (verts, tris)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::SurfaceMap;
    use ed_geo::{EnuFrame, Geodetic};

    #[test]
    fn sample_matches_vertices_and_is_continuous() {
        let tile = TileId::new(14, 8544, 5827);
        let mut g = TerrainGrid::flat(tile, 4, 0.0);
        for (k, h) in g.heights.iter_mut().enumerate() {
            *h = (k * 7 % 13) as f32;
        }
        for j in 0..=4 {
            for i in 0..=4 {
                let s = g.sample(i as f64 / 4.0, j as f64 / 4.0);
                assert!((s - g.h(i, j)).abs() < 1e-3, "({i},{j}) {s} vs {}", g.h(i, j));
            }
        }
        // Diagonal continuity
        let a = g.sample(0.125 + 1e-7, 0.125);
        let b = g.sample(0.125, 0.125 + 1e-7);
        assert!((a - b).abs() < 1e-3);
    }

    #[test]
    fn mesh_faces_up() {
        let tile = TileId::containing(46.02, 7.75, 14);
        let frame = EnuFrame::new(Geodetic::new(46.02, 7.75, 0.0));
        let proj = TileProjector::new(tile, frame);
        let g = TerrainGrid::flat(tile, 8, 1600.0);
        let surface = SurfaceMap::uniform(tile, crate::biome::Biome::Alpine);
        let m = build_mesh(&g, &proj, &surface, 0.0);
        assert_eq!(m.triangle_count(), 8 * 8 * 2);
        let up = proj.up();
        // First triangle's geometric normal must face up (CCW from above).
        let [a, b, c] = [m.indices[0], m.indices[1], m.indices[2]].map(|i| Vec3::from(m.positions[i as usize]));
        let n = (b - a).cross(c - a).normalize();
        assert!(n.dot(up) > 0.99, "normal {n:?} up {up:?}");
        // Tile spans ~1.7 km at this latitude.
        let w = Vec3::from(m.positions[8]).distance(Vec3::from(m.positions[0]));
        assert!((1600.0..1800.0).contains(&w), "{w}");
    }
}
