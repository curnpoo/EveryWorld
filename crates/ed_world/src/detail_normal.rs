//! Baked terrain detail: what the DEM knows that the mesh can't show.
//!
//! Far tiles mesh the DEM coarsely (a z9 quad is ~800 m), so ridges, gullies
//! and couloirs vanish into smooth blobs. This bakes, per texel, the
//! difference between the DEM's fine slope and the slope the mesh's vertex
//! normals interpolate, plus the fine curvature. The shader adds the slope
//! delta to the vertex normal (heightfield form, so it is exact for any mesh
//! slope) and later phases use the curvature for cavities, ridges and snow.
//!
//! Layout matches the satellite texture: texel `i` sits on u = i / (n-1), so
//! texels land on mesh vertices at the tile edges and neighbours agree.
//! RGBA8: R/G = east/south slope delta (`0.5 + atan(d)/pi`), B = curvature
//! (same mapping, + = concave gully, - = convex ridge), A = fine slope
//! magnitude (`atan(|grad|)/(pi/2)`).

use crate::amplify::Amplifier;
use crate::terrain::TerrainGrid;
use ed_data::elevation::HeightTile;
use std::f32::consts::PI;

#[inline]
fn enc(x: f32) -> u8 {
    ((0.5 + x.atan() / PI) * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Inverse of the R/G/B encoding (what the shader does), for tests.
pub fn dec(b: u8) -> f32 {
    ((b as f32 / 255.0).clamp(0.02, 0.98) - 0.5).mul_add(PI, 0.0).tan()
}

/// East/south height gradient of the mesh as its smooth vertex normals render
/// it: central differences on the grid (one-sided at edges), bilinear between.
fn coarse_grad(grid: &TerrainGrid, w_m: f32) -> Vec<[f32; 2]> {
    let s = grid.segments;
    let n = s + 1;
    let cell = w_m / s as f32;
    (0..n * n)
        .map(|k| {
            let (i, j) = (k % n, k / n);
            let (l, r) = (i.saturating_sub(1), (i + 1).min(s));
            let (t, b) = (j.saturating_sub(1), (j + 1).min(s));
            let ge = (grid.h(r, j) - grid.h(l, j)) / ((r - l) as f32 * cell);
            let gs = (grid.h(i, b) - grid.h(i, t)) / ((b - t) as f32 * cell);
            [ge, gs]
        })
        .collect()
}

/// Bake an `n`x`n` detail texture for `grid.tile` from `src` (the tile's DEM or
/// an ancestor's), relative to the coarse surface `grid`. `w_m` = tile width.
pub fn bake(grid: &TerrainGrid, src: &HeightTile, n: usize, w_m: f64) -> Vec<u8> {
    bake_with(grid, &Amplifier::new(grid.tile, src), n, w_m, false)
}

/// As [`bake`], with the fine surface = DEM + amplification (`amplify`) when
/// `amplified` (band-limited to the texel, like the grid to its cells).
pub fn bake_with(grid: &TerrainGrid, amp: &Amplifier, n: usize, w_m: f64, amplified: bool) -> Vec<u8> {
    let src = amp.src();
    let (u0, v0) = amp.to_src(0.0, 0.0);
    let sc = amp.to_src(1.0, 0.0).0 - u0;
    let cg = coarse_grad(grid, w_m as f32);
    let gs = grid.segments;
    let gn = gs + 1;
    // Finite-difference step: one DEM pixel, but never finer than a texel
    // (otherwise the texture aliases the DEM instead of filtering it).
    let dem_px = 1.0 / ((src.size.max(2) - 1) as f64 * sc);
    let texel = 1.0 / (n.max(2) - 1) as f64;
    let e = dem_px.max(texel);
    let e_m = (e * w_m) as f32;
    // Sample in source space so ancestor DEMs reach past the tile edge; only
    // the DEM's own edge clamps, and then the step shrinks to match.
    let ae = e * sc;
    let min_l = 2.0 * e_m as f64;
    let hs = |a: f64, b: f64| if amplified { amp.height_src(a, b, min_l) } else { src.sample_cubic(a, b) };
    let mut out = Vec::with_capacity(n * n * 4);
    for j in 0..n {
        for i in 0..n {
            let (u, v) = (i as f64 * texel, j as f64 * texel);
            let (a, b) = (u0 + sc * u, v0 + sc * v);
            let (aw, ae_) = ((a - ae).max(0.0), (a + ae).min(1.0));
            let (bn, bs) = ((b - ae).max(0.0), (b + ae).min(1.0));
            let (hw, he) = (hs(aw, b), hs(ae_, b));
            let (hn, hsth) = (hs(a, bn), hs(a, bs));
            let m = (w_m / sc) as f32; // source-space metres per unit
            let fe = (he - hw) / (((ae_ - aw) as f32) * m).max(1e-3);
            let fs = (hsth - hn) / (((bs - bn) as f32) * m).max(1e-3);
            // Curvature: centre the stencil inside the DEM.
            let (ac, bc) = (a.clamp(ae, 1.0 - ae), b.clamp(ae, 1.0 - ae));
            let curv = (hs(ac - ae, bc) + hs(ac + ae, bc) + hs(ac, bc - ae) + hs(ac, bc + ae) - 4.0 * hs(ac, bc)) / e_m;
            // Coarse gradient, bilinear on the grid like the vertex normals.
            let fx = (u * gs as f64).min(gs as f64 - 1e-9);
            let fy = (v * gs as f64).min(gs as f64 - 1e-9);
            let (gi, gj) = (fx.floor() as usize, fy.floor() as usize);
            let (tx, ty) = ((fx - gi as f64) as f32, (fy - gj as f64) as f32);
            let at = |a: usize, b: usize| cg[b * gn + a];
            let lerp2 = |c: usize| {
                let a = at(gi, gj)[c] + (at(gi + 1, gj)[c] - at(gi, gj)[c]) * tx;
                let b = at(gi, gj + 1)[c] + (at(gi + 1, gj + 1)[c] - at(gi, gj + 1)[c]) * tx;
                a + (b - a) * ty
            };
            let (ce, cs) = (lerp2(0), lerp2(1));
            let slope = (fe * fe + fs * fs).sqrt();
            out.extend_from_slice(&[enc(fe - ce), enc(fs - cs), enc(curv), ((slope.atan() / (PI * 0.5)) * 255.0).round() as u8]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_geo::tile::TileId;

    fn dem(id: TileId, size: usize, f: impl Fn(f64, f64) -> f32) -> HeightTile {
        let s = (size - 1) as f64;
        let heights = (0..size * size).map(|k| f((k % size) as f64 / s, (k / size) as f64 / s)).collect();
        HeightTile { id, size, heights }
    }

    #[test]
    fn plane_has_no_detail() {
        let t = TileId::new(10, 533, 360);
        let w = t.width_m();
        // A uniform east-facing slope: the coarse mesh already has it exactly.
        let d = dem(t, 256, |u, _| (u * w * 0.4) as f32);
        let g = TerrainGrid::from_heights(t, &d, 16);
        let tex = bake(&g, &d, 32, w);
        for px in tex.chunks(4) {
            assert!(dec(px[0]).abs() < 0.03 && dec(px[1]).abs() < 0.03, "{px:?}");
            assert!(dec(px[2]).abs() < 0.03);
            assert!((px[3] as f32 / 255.0 - 0.4f32.atan() / (PI * 0.5)).abs() < 0.02);
        }
    }

    #[test]
    fn fine_ridges_survive_a_coarse_mesh() {
        let t = TileId::new(9, 266, 180);
        let w = t.width_m();
        // 16 ridges across the tile: invisible to a 4-quad mesh.
        let d = dem(t, 512, |u, _| (300.0 * (u * 16.0 * std::f64::consts::TAU).sin()) as f32);
        let g = TerrainGrid::from_heights(t, &d, 4);
        let tex = bake(&g, &d, 256, w);
        let max_de = tex.chunks(4).map(|p| dec(p[0]).abs()).fold(0.0, f32::max);
        // Peak slope = 300 * 2pi * 16 / w.
        let want = (300.0 * std::f64::consts::TAU * 16.0 / w) as f32;
        assert!(max_de > want * 0.7, "{max_de} vs {want}");
        // Crests are convex (curvature < 0), troughs concave.
        let row = |i: usize| &tex[i * 4..i * 4 + 4];
        let crest = (0.25f64 / 16.0 * 255.0).round() as usize; // sin peak at u = 1/64
        assert!(dec(row(crest)[2]) < 0.0 && dec(row(crest + 8)[2]) > 0.0);
    }

    #[test]
    fn ancestor_dem_maps_to_tile() {
        let t = TileId::new(11, 1066, 721);
        let (a, ..) = t.ancestor(9);
        let w = t.width_m();
        let d = dem(a, 256, |u, v| (u * 1000.0 + v * 500.0) as f32);
        let g = TerrainGrid::from_heights(t, &d, 8);
        let tex = bake(&g, &d, 16, w);
        assert_eq!(tex.len(), 16 * 16 * 4);
        for px in tex.chunks(4) {
            assert!(dec(px[0]).abs() < 0.03 && dec(px[1]).abs() < 0.03);
        }
    }
}
