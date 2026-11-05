//! Lakes and rivers from the OpenMapTiles `water` layer. The open ocean is
//! drawn globally by the game (a curved sea-level shell), so here we only
//! handle inland water, and carve the terrain under all water so the surface
//! never z-fights with the ground.

use crate::mesh::{MeshData, rgb};
use crate::project::TileProjector;
use crate::surface::SurfaceMap;
use crate::terrain::TerrainGrid;
use ed_data::mvt::{self, GeomType};
use glam::Vec3;

/// Push the ground under mapped water down so it sits below the surface.
pub fn carve_terrain(grid: &mut TerrainGrid, surface: &SurfaceMap, depth: f32) {
    let n = grid.segments + 1;
    for j in 0..n {
        for i in 0..n {
            let (u, v) = (i as f64 / grid.segments as f64, j as f64 / grid.segments as f64);
            // Smooth ramp over the filtered mask: the bank slopes into the bed instead of dropping a
            // cover-cell-sized step, so the waterline (terrain crossing the water plane) is a clean curve.
            let f = surface.water_fraction(u, v);
            let k = ((f - 0.2) / 0.6).clamp(0.0, 1.0);
            let k = k * k * (3.0 - 2.0 * k);
            if k > 0.0 {
                grid.heights[j * n + i] -= depth * k;
            }
        }
    }
}

/// Inland water surfaces. Each polygon is flat at the lowest shore height.
pub fn generate(vt: &mvt::Tile, proj: &TileProjector, grid_uncarved: &TerrainGrid) -> MeshData {
    let mut m = MeshData::default();
    let Some(layer) = vt.layer("water") else { return m };
    let ext = layer.extent as f64;
    let up = proj.up();
    let color = rgb(0x3f7f9a);
    for f in &layer.features {
        if f.geom_type != GeomType::Polygon || f.str("class") == Some("ocean") {
            continue;
        }
        for (outer, holes) in mvt::polygons(f) {
            let uv = |p: &[f32; 2]| (p[0] as f64 / ext, p[1] as f64 / ext);
            let level = outer
                .iter()
                .map(|p| { let (u, v) = uv(p); grid_uncarved.sample(u, v) })
                .fold(f32::MAX, f32::min)
                .max(0.3) as f64;
            let mut flat = Vec::new();
            let mut pts = Vec::new();
            let mut hole_idx = Vec::new();
            for (k, ring) in std::iter::once(&outer).chain(holes.iter()).enumerate() {
                if k > 0 {
                    hole_idx.push(pts.len());
                }
                for p in ring {
                    let (u, v) = uv(p);
                    flat.extend_from_slice(&[u, v]);
                    pts.push(proj.local(u, v, level));
                }
            }
            let Ok(tris) = earcutr::earcut(&flat, &hole_idx, 2) else { continue };
            let base = m.positions.len() as u32;
            for p in &pts {
                m.vertex(p.to_array(), up.to_array(), [p.x * 0.05, p.z * 0.05], color);
            }
            for t in tris.chunks_exact(3) {
                let (a, b, c) = (t[0] as u32 + base, t[1] as u32 + base, t[2] as u32 + base);
                let (pa, pb, pc) = (pts[t[0]], pts[t[1]], pts[t[2]]);
                if (pb - pa).cross(pc - pa).dot(up) >= 0.0 { m.tri(a, b, c) } else { m.tri(a, c, b) }
            }
        }
    }
    let _ = Vec3::ZERO;
    m
}
