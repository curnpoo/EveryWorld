//! Near-camera cliff kits: faceted rock slabs on steep faces.
//!
//! Even an amplified 9 m grid draws a cliff as a smooth sheet. Up close,
//! real faces are blocky: overhangs, ledges, slabs. This scatters chunky,
//! flat-shaded slabs over steep ground on fine tiles. Each slab stands upright
//! with its thin axis along the face's downhill direction and sits half in the
//! slope, so the terrain reads as stacked rock rather than a draped sheet. Slabs
//! are pure decoration: the collider stays the terrain grid.

use crate::foliage::RockInstance;
use crate::mesh::{MeshData, rgb, scale_color};
use crate::project::TileProjector;
use crate::surface::SurfaceMap;
use crate::terrain::TerrainGrid;
use glam::Vec3;

/// Spacing of slab candidates (m).
const CELL_M: f32 = 14.0;
/// Rise/run where slabs start / are dense (~40 and ~55 degrees).
const STEEP: (f32, f32) = (0.85, 1.4);

/// A unit slab ([-0.5, 0.5]^3) with jittered corners, a bevelled top and flat
/// shading (every triangle owns its vertices), so faces catch light as facets.
pub fn slab_mesh(c: [f32; 4]) -> MeshData {
    let mut corners = [Vec3::ZERO; 10];
    let mut k = 0;
    for &y in &[-0.5f32, 0.5] {
        for &(x, z) in &[(-0.5f32, -0.5f32), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
            let j = |s: u64| (crate::hash01(k as u64, s) - 0.5) * 0.22;
            corners[k] = Vec3::new(x + j(1), y + j(2), z + j(3));
            k += 1;
        }
    }
    // Bevel ridge on top, offset so the slab looks fractured, not boxy.
    corners[8] = Vec3::new(-0.2, 0.62, 0.08);
    corners[9] = Vec3::new(0.25, 0.58, -0.05);
    let tris: [[usize; 3]; 20] = [
        // bottom
        [0, 2, 1], [0, 3, 2],
        // sides (CCW from outside)
        [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7],
        // top: two ridge points over the quad 4-5-6-7
        [4, 5, 9], [4, 9, 8], [5, 6, 9], [6, 7, 8], [6, 8, 9], [7, 4, 8],
        // fill the remaining top slivers (duplicates are harmless, keep closed)
        [4, 8, 7], [5, 9, 6], [8, 9, 6], [4, 9, 5],
    ];
    let mut m = MeshData::default();
    for (ti, t) in tris.iter().enumerate().take(16) {
        let (a, b, cc) = (corners[t[0]], corners[t[1]], corners[t[2]]);
        let n = (b - a).cross(cc - a).normalize_or_zero();
        // Slight per-facet tone so facets separate even in flat light.
        let f = 0.88 + 0.24 * crate::hash01(ti as u64, 77);
        let col = [c[0] * f, c[1] * f, c[2] * f, 1.0];
        let base = m.positions.len() as u32;
        for p in [a, b, cc] {
            m.vertex(p.to_array(), n.to_array(), [0.0; 2], col);
        }
        m.tri(base, base + 1, base + 2);
    }
    m
}

/// Slab instances over the steep parts of `grid` (fine tiles only).
pub fn scatter(proj: &TileProjector, grid: &TerrainGrid, surface: &SurfaceMap) -> Vec<RockInstance> {
    let width = proj.tile.width_m() as f32;
    let cells = ((width / CELL_M) as usize).clamp(8, 256);
    let du = 1.0 / cells as f64;
    let e = (1.0 / grid.segments as f64).max(du * 0.5);
    let tkey = (proj.tile.x as u64) << 32 | proj.tile.y as u64 | (proj.tile.z as u64) << 58 | 0xC1;
    let em = (2.0 * e) as f32 * width;
    let tint = rgb(match surface.lith {
        Some(l) => l.rock_rgb(),
        None => 0x8f8a82,
    });
    let mut out = Vec::new();
    for j in 0..cells {
        for i in 0..cells {
            let key = (j * cells + i) as u64;
            let (ju, jv) = (crate::hash01(tkey, key * 5), crate::hash01(tkey, key * 5 + 1));
            let u = (i as f64 + ju as f64) * du;
            let v = (j as f64 + jv as f64) * du;
            if surface.cover_at(u, v).is_urban() {
                continue;
            }
            let ge = (grid.sample(u + e, v) - grid.sample(u - e, v)) / em;
            let gs = (grid.sample(u, v + e) - grid.sample(u, v - e)) / em;
            let g = (ge * ge + gs * gs).sqrt();
            let p = crate::noise::smoothstep(STEEP.0, STEEP.1, g);
            if crate::hash01(tkey, key * 5 + 2) > p * 0.9 {
                continue;
            }
            let r = crate::hash01(tkey, key * 5 + 3);
            let (w, h, d) = (6.0 + 12.0 * r, 4.0 + 10.0 * crate::hash01(tkey, key * 5 + 4), 2.0 + 3.0 * r);
            // Thin axis (model z) points downhill: render x east, z south.
            let yaw = (-ge).atan2(-gs) + (ju - 0.5) * 0.5;
            // Sink the slab into the slope: centre slightly uphill and below
            // the surface so its downhill face stands proud like a ledge.
            let hz = grid.sample(u, v) as f64;
            let back = Vec3::new(ge, 0.0, gs) / g.max(1e-3) * (d * 0.25);
            let pos = proj.local(u, v, hz - 0.3 * h as f64) + back;
            out.push(RockInstance { pos, scale: Vec3::new(w, h, d), yaw, tint: scale_color(tint, 0.75 + 0.35 * jv) });
        }
    }
    out
}

/// Bake slabs into culled chunks, like `props::bake_rocks_chunked`.
pub fn bake_chunked(slabs: &[RockInstance], up: Vec3, width: f32, grid: usize) -> Vec<MeshData> {
    crate::props::bake_instances_chunked(slabs, &slab_mesh([1.0; 4]), up, width, grid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::Biome;
    use ed_geo::tile::TileId;

    fn setup(f: impl Fn(f64, f64) -> f32) -> (TileProjector, TerrainGrid, SurfaceMap) {
        let t = TileId::new(14, 8545, 5826);
        let frame = ed_geo::EnuFrame::new(t.center());
        let proj = TileProjector::new(t, frame);
        let s = 32;
        let w = t.width_m();
        let heights = (0..(s + 1) * (s + 1)).map(|k| f((k % (s + 1)) as f64 / s as f64 * w, (k / (s + 1)) as f64 / s as f64 * w)).collect();
        (proj, TerrainGrid { tile: t, segments: s, heights, holes: Vec::new() }, SurfaceMap::uniform(t, Biome::Alpine))
    }

    #[test]
    fn flat_ground_gets_no_slabs() {
        let (p, g, s) = setup(|_, _| 2500.0);
        assert!(scatter(&p, &g, &s).is_empty());
    }

    #[test]
    fn cliffs_get_downhill_facing_slabs() {
        // Rises 1.6 m per m to the east: downhill is west (-x).
        let (p, g, s) = setup(|x, _| 2000.0 + 1.6 * x as f32);
        let a = scatter(&p, &g, &s);
        assert!(a.len() > 100, "{}", a.len());
        assert_eq!(a.len(), scatter(&p, &g, &s).len());
        for r in &a {
            let facing = Vec3::new(r.yaw.sin(), 0.0, r.yaw.cos());
            assert!(facing.x < -0.8, "{facing:?}");
        }
        let m = slab_mesh([1.0; 4]);
        assert_eq!(m.indices.len(), 16 * 3);
    }
}
