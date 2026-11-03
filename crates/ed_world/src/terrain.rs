//! Terrain tile meshes.
//!
//! A [`TerrainGrid`] is the single source of truth for ground height inside a
//! tile: the render mesh, the physics collider, and everything that sits on
//! the ground (buildings, roads, trees, water) all sample the *same* grid with
//! the *same* triangle interpolation, so nothing floats or sinks.

use crate::mesh::MeshData;
use crate::project::TileProjector;
use crate::surface::{SurfaceMap, pack_splat};
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
    pub fn h(&self, i: usize, j: usize) -> f32 {
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

/// Render-mesh subdivision per grid quad. Finer tiles get extra vertices so
/// the per-vertex ground splat can follow the ~9 m land-cover grid. The
/// extra vertices lie exactly on the grid's triangles (same diagonal), so
/// the render surface still matches the collider and `sample`.
pub fn subdivision_for(grid: &TerrainGrid, surface: &SurfaceMap) -> usize {
    let want = if grid.tile.z >= 14 { 3 } else if grid.tile.z == 13 { 2 } else { 1 };
    want.min((surface.res / grid.segments.max(1)).max(1))
}

/// Build the render mesh. Vertex colors carry the regional vegetation tint
/// (+ wetness in alpha) and `MeshData::splat` the ground-layer weights from
/// the surface map (land cover + biome + slope + snowline). `skirt_m` hangs
/// a curtain off each edge to hide cracks between neighbouring tiles at
/// different LODs.
pub fn build_mesh(grid: &TerrainGrid, proj: &TileProjector, surface: &SurfaceMap, skirt_m: f32) -> MeshData {
    build_mesh_sub(grid, proj, surface, skirt_m, subdivision_for(grid, surface))
}

pub fn build_mesh_sub(grid: &TerrainGrid, proj: &TileProjector, surface: &SurfaceMap, skirt_m: f32, sub: usize) -> MeshData {
    let gseg = grid.segments;
    let gn = gseg + 1;
    let sub = sub.max(1);
    let seg = gseg * sub;
    let n = seg + 1;
    let mut m = MeshData::default();
    // Smooth normals on the coarse grid, bilinearly spread to sub-vertices.
    let gpos: Vec<Vec3> = (0..gn * gn)
        .map(|k| {
            let (i, j) = (k % gn, k / gn);
            proj.local(i as f64 / gseg as f64, j as f64 / gseg as f64, grid.h(i, j) as f64)
        })
        .collect();
    let up = proj.up();
    let gnormal: Vec<Vec3> = (0..gn * gn)
        .map(|k| {
            let (i, j) = (k % gn, k / gn);
            let l = gpos[j * gn + i.saturating_sub(1)];
            let r = gpos[j * gn + (i + 1).min(gseg)];
            let t = gpos[j.saturating_sub(1) * gn + i];
            let b = gpos[(j + 1).min(gseg) * gn + i];
            let nrm = (r - l).cross(t - b).normalize_or_zero();
            if nrm.dot(up) < 0.0 { -nrm } else { nrm }
        })
        .collect();
    let normal_at = |fx: f64, fy: f64| -> Vec3 {
        let (i, j) = ((fx.floor() as usize).min(gseg.saturating_sub(1)), (fy.floor() as usize).min(gseg.saturating_sub(1)));
        let (tx, ty) = ((fx - i as f64) as f32, (fy - j as f64) as f32);
        let a = gnormal[j * gn + i].lerp(gnormal[j * gn + i + 1], tx);
        let b = gnormal[(j + 1) * gn + i].lerp(gnormal[(j + 1) * gn + i + 1], tx);
        a.lerp(b, ty).normalize_or_zero()
    };
    m.positions.reserve(n * n);
    m.splat.reserve(n * n);
    for j in 0..n {
        for i in 0..n {
            let (u, v) = (i as f64 / seg as f64, j as f64 / seg as f64);
            let h = if i % sub == 0 && j % sub == 0 { grid.h(i / sub, j / sub) } else { grid.sample(u, v) };
            let p = if sub == 1 { gpos[j * gn + i] } else { proj.local(u, v, h as f64) };
            let nrm = normal_at(u * gseg as f64, v * gseg as f64);
            let slope = 1.0 - nrm.dot(up).clamp(0.0, 1.0);
            let w = surface.splat(u, v, h, slope);
            m.vertex(p.to_array(), nrm.to_array(), [u as f32, v as f32], surface.tint(u, v));
            m.splat.push(pack_splat(&w));
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
                m.splat.push(m.splat[k]);
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
        let m = build_mesh_sub(&g, &proj, &surface, 0.0, 1);
        assert_eq!(m.triangle_count(), 8 * 8 * 2);
        assert_eq!(m.splat.len(), m.positions.len());
        let up = proj.up();
        // First triangle's geometric normal must face up (CCW from above).
        let [a, b, c] = [m.indices[0], m.indices[1], m.indices[2]].map(|i| Vec3::from(m.positions[i as usize]));
        let n = (b - a).cross(c - a).normalize();
        assert!(n.dot(up) > 0.99, "normal {n:?} up {up:?}");
        // Tile spans ~1.7 km at this latitude.
        let w = Vec3::from(m.positions[8]).distance(Vec3::from(m.positions[0]));
        assert!((1600.0..1800.0).contains(&w), "{w}");
    }

    #[test]
    fn subdivided_mesh_lies_on_grid_triangles() {
        let tile = TileId::containing(46.02, 7.75, 14);
        let frame = EnuFrame::new(Geodetic::new(46.02, 7.75, 0.0));
        let proj = TileProjector::new(tile, frame);
        let mut g = TerrainGrid::flat(tile, 4, 0.0);
        for (k, h) in g.heights.iter_mut().enumerate() {
            *h = 1500.0 + (k * 37 % 11) as f32 * 9.0;
        }
        let mut surface = SurfaceMap::uniform(tile, crate::biome::Biome::Alpine);
        surface.res = 12;
        surface.cover = vec![crate::surface::Cover::Grass; 144];
        let m = build_mesh(&g, &proj, &surface, 3.0);
        let n = 4 * 3 + 1;
        assert_eq!(m.splat.len(), m.positions.len());
        for j in 0..n {
            for i in 0..n {
                let (u, v) = (i as f64 / 12.0, j as f64 / 12.0);
                let want = proj.local(u, v, g.sample(u, v) as f64);
                let got = Vec3::from(m.positions[j * n + i]);
                assert!(got.distance(want) < 0.05, "({i},{j}) {got:?} vs {want:?}");
                let s: u32 = m.splat[j * n + i].iter().map(|&b| b as u32).sum();
                assert!((250..=262).contains(&s), "splat sum {s}");
            }
        }
    }
}

#[cfg(test)]
mod sub_tests {
    use super::*;
    use ed_geo::{EnuFrame, Geodetic};

    #[test]
    fn no_long_triangles_any_subdivision() {
        let tile = TileId::containing(40.78, -73.97, 15);
        let proj = TileProjector::new(tile, EnuFrame::new(Geodetic::new(40.78, -73.97, 0.0)));
        for (segs, res) in [(24usize, 48usize), (64, 192), (8, 8)] {
            let mut g = TerrainGrid::flat(tile, segs, 120.0);
            for (k, h) in g.heights.iter_mut().enumerate() {
                *h = 120.0 + (k % 7) as f32;
            }
            let mut s = SurfaceMap::uniform(tile, crate::biome::Biome::TemperateForest);
            s.res = res;
            s.cover = vec![crate::surface::Cover::Grass; res * res];
            let m = build_mesh(&g, &proj, &s, 6.0);
            let max_edge = tile.width_m() as f32 / segs as f32 * 1.6 + 7.0;
            for t in m.indices.chunks_exact(3) {
                let p: Vec<Vec3> = t.iter().map(|&i| Vec3::from(m.positions[i as usize])).collect();
                for k in 0..3 {
                    let e = p[k].distance(p[(k + 1) % 3]);
                    assert!(e < max_edge, "segs {segs} res {res}: edge {e} > {max_edge} in {t:?}");
                }
            }
        }
    }
}
