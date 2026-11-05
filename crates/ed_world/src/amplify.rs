//! Terrain amplification: erosion-style relief finer than the DEM.
//!
//! 30 m DEMs smooth alpine faces into ramps. Real faces are ribbed with
//! gullies and buttresses at 20-300 m. This adds ridged noise (alternating
//! crests and gullies per octave) on steep, high ground only, so valley
//! floors, meadows and towns keep the DEM untouched. It is a pure function
//! of global position + the DEM, so neighbouring tiles agree at seams, and
//! it is band-limited per caller (`min_lambda`) so a coarse mesh or texture
//! never point-samples noise it can't resolve.
//!
//! The result goes into `TerrainGrid` heights (mesh, collider, buildings all
//! see it) and into the detail-normal bake.

use crate::biome::Biome;
use crate::noise::{smoothstep, value};
use ed_data::elevation::HeightTile;
use ed_geo::tile::TileId;

/// Octave wavelengths (m) and amplitude as a fraction of wavelength.
const LAMBDAS: [f64; 5] = [320.0, 160.0, 80.0, 40.0, 20.0];
const AMP: f32 = 0.05;
/// Slope (rise/run) where soft relief starts / is full: hillsides, not
/// valley floors (below ~5 degrees nothing changes).
const GENTLE: (f32, f32) = (0.08, 0.30);
/// Slope where relief turns sharp (ridged crests and gullies).
const SLOPE: (f32, f32) = (0.35, 0.9);
/// Share of the full amplitude kept on low ground (well below the snowline).
const LOW_AMP: f32 = 0.4;

/// Equatorial ground width of one z14 tile (m).
const W14: f64 = 2.0 * std::f64::consts::PI * ed_geo::WGS84_A / 16384.0;

pub struct Amplifier<'a> {
    tile: TileId,
    src: &'a HeightTile,
    u0: f64,
    v0: f64,
    sc: f64,
    /// Tile width in metres.
    w_m: f64,
    /// Gradient step in source uv (fixed metres, so every LOD agrees on slope).
    step: f64,
    snowline: f32,
}

impl<'a> Amplifier<'a> {
    /// `src` is this tile's DEM or an ancestor's.
    pub fn new(tile: TileId, src: &'a HeightTile) -> Self {
        let (u0, v0, sc) = if src.id == tile { (0.0, 0.0, 1.0) } else {
            let (_, u0, v0, s) = tile.ancestor(src.id.z);
            (u0, v0, s)
        };
        let w_m = tile.width_m();
        let src_m = w_m / sc;
        let dem_px = 1.0 / (src.size.max(2) - 1) as f64;
        let step = dem_px.max(45.0 / src_m);
        let snowline = Biome::snowline_m(tile.center().lat_deg);
        Self { tile, src, u0, v0, sc, w_m, step, snowline }
    }

    pub fn tile_width_m(&self) -> f64 {
        self.w_m
    }

    /// Source-space coordinates of tile (u, v) (may lie outside the tile).
    #[inline]
    pub fn to_src(&self, u: f64, v: f64) -> (f64, f64) {
        (self.u0 + self.sc * u, self.v0 + self.sc * v)
    }

    /// (amplitude weight, sharpness) 0..1 at source (a, b) with DEM height `h`.
    fn weight(&self, a: f64, b: f64, h: f32) -> (f32, f32) {
        let e = self.step;
        let (aw, ae) = ((a - e).max(0.0), (a + e).min(1.0));
        let (bn, bs) = ((b - e).max(0.0), (b + e).min(1.0));
        let m = (self.w_m / self.sc) as f32;
        let ge = (self.src.sample_cubic(ae, b) - self.src.sample_cubic(aw, b)) / (((ae - aw) as f32) * m).max(1e-3);
        let gs = (self.src.sample_cubic(a, bs) - self.src.sample_cubic(a, bn)) / (((bs - bn) as f32) * m).max(1e-3);
        let slope = (ge * ge + gs * gs).sqrt();
        let high = smoothstep(self.snowline - 1700.0, self.snowline - 700.0, h);
        // Walls (> ~55 degrees): vertical displacement of sideways noise turns
        // into vertical ribs/streaks down the face, so leave them as the DEM has them.
        let wall = 1.0 - smoothstep(1.4, 2.6, slope);
        let w = smoothstep(GENTLE.0, GENTLE.1, slope) * (LOW_AMP + (1.0 - LOW_AMP) * high) * wall;
        (w, smoothstep(SLOPE.0, SLOPE.1, slope) * high)
    }

    /// Height offset (m) at source (a, b) for DEM height `h`, keeping only
    /// octaves with wavelength >= ~`min_lambda` metres.
    pub fn delta_src(&self, a: f64, b: f64, h: f32, min_lambda: f64) -> f32 {
        if LAMBDAS[0] < min_lambda {
            return 0.0;
        }
        let (w, sharp) = self.weight(a, b, h);
        if w <= 0.0 {
            return 0.0;
        }
        // Global z14 units -> metres (local ground scale).
        let k = 2f64.powi(14 - self.src.id.z as i32);
        let (gx, gy) = ((self.src.id.x as f64 + a) * k, (self.src.id.y as f64 + b) * k);
        let lat = ed_geo::tile::TileId::new(14, 0, 0).uv_to_geodetic(0.0, gy).lat_deg;
        let s = W14 * lat.to_radians().cos();
        let (xm, ym) = (gx * s, gy * s);
        let mut sum = 0.0f32;
        for (o, &l) in LAMBDAS.iter().enumerate() {
            let fade = smoothstep(min_lambda as f32, 2.0 * min_lambda as f32, l as f32);
            if fade <= 0.0 {
                continue;
            }
            // Rotate each octave so value-noise grid axes don't line up.
            let ang = 0.7 + o as f64 * 1.13;
            let (c, sn) = (ang.cos(), ang.sin());
            let (x, y) = ((xm * c - ym * sn) / l, (xm * sn + ym * c) / l);
            let nv = value(x, y, 9100 + o as u64, 0);
            // Hillsides: rounded swells and runnels. Steep high faces: ridged,
            // even octaves raising crests and odd ones cutting gullies.
            let soft = (nv - 0.5) * 2.0;
            let sgn = if o % 2 == 0 { 1.0 } else { -1.0 };
            let ridged = sgn * ((1.0 - (2.0 * nv - 1.0).abs()) - 0.5) * 2.0;
            sum += (soft + (ridged - soft) * sharp) * AMP * l as f32 * fade;
        }
        sum * w
    }

    /// Height offset at tile (u, v).
    pub fn delta(&self, u: f64, v: f64, h: f32, min_lambda: f64) -> f32 {
        let (a, b) = self.to_src(u, v);
        self.delta_src(a.clamp(0.0, 1.0), b.clamp(0.0, 1.0), h, min_lambda)
    }

    /// Amplified height at source (a, b).
    pub fn height_src(&self, a: f64, b: f64, min_lambda: f64) -> f32 {
        let h = self.src.sample_cubic(a, b);
        h + self.delta_src(a, b, h, min_lambda)
    }

    pub fn tile(&self) -> TileId {
        self.tile
    }

    pub fn src(&self) -> &'a HeightTile {
        self.src
    }
}

/// Amplify a grid in place (band-limited to its cell size).
pub fn apply(grid: &mut crate::terrain::TerrainGrid, amp: &Amplifier) {
    let s = grid.segments;
    let min_l = 2.0 * amp.tile_width_m() / s as f64;
    let n = s + 1;
    for j in 0..n {
        for i in 0..n {
            let (u, v) = (i as f64 / s as f64, j as f64 / s as f64);
            let h = grid.heights[j * n + i];
            grid.heights[j * n + i] = h + amp.delta(u, v, h, min_l);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::TerrainGrid;

    fn dem(id: TileId, f: impl Fn(f64, f64) -> f32) -> HeightTile {
        let size = 256;
        let s = (size - 1) as f64;
        HeightTile { id, size, heights: (0..size * size).map(|k| f((k % size) as f64 / s, (k / size) as f64 / s)).collect() }
    }

    // Zermatt area.
    fn alps(z: u8) -> TileId {
        let (x, y) = ((7.75 + 180.0) / 360.0, {
            let lat = 46.0f64.to_radians();
            (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / std::f64::consts::PI) / 2.0
        });
        let n = (1u32 << z) as f64;
        TileId::new(z, (x * n) as u32, (y * n) as u32)
    }

    #[test]
    fn flat_ground_is_untouched_and_low_ground_softer() {
        let t = alps(13);
        let w = t.width_m();
        // Steep but low: softer relief than the same face up high.
        let low = dem(t, |u, _| (u * w * 1.2) as f32);
        let a = Amplifier::new(t, &low);
        let amp = |h: f32| (0..200).map(|i| a.delta(0.3 + i as f64 * 0.002, 0.5, h, 1.0).abs()).fold(0.0f32, f32::max);
        assert!(amp(300.0) > 1.0 && amp(300.0) < amp(3200.0), "{} {}", amp(300.0), amp(3200.0));
        // High but flat: untouched.
        let flat = dem(t, |_, _| 3300.0);
        let a = Amplifier::new(t, &flat);
        assert_eq!(a.delta(0.5, 0.5, 3300.0, 1.0), 0.0);
    }

    #[test]
    fn steep_high_faces_get_bounded_relief() {
        let t = alps(13);
        let w = t.width_m();
        let face = dem(t, |u, _| 2800.0 + (u * w * 1.2) as f32);
        let a = Amplifier::new(t, &face);
        let ds: Vec<f32> = (0..200).map(|i| a.delta(0.3 + i as f64 * 0.002, 0.5, 3200.0, 1.0)).collect();
        let max = ds.iter().fold(0.0f32, |m, d| m.max(d.abs()));
        assert!(max > 5.0 && max < 32.0, "{max}");
        // Band-limiting: a coarse caller sees less (and nothing below 320 m).
        let coarse = (0..200).map(|i| a.delta(0.3 + i as f64 * 0.002, 0.5, 3200.0, 200.0).abs()).fold(0.0f32, f32::max);
        assert!(coarse < max);
        assert_eq!(a.delta(0.5, 0.5, 3200.0, 1000.0), 0.0);
    }

    #[test]
    fn seams_agree_between_neighbours_and_ancestors() {
        let t = alps(13);
        let right = TileId::new(t.z, t.x + 1, t.y);
        let parent = t.parent().unwrap();
        // One continuous face expressed in the parent's DEM.
        let pw = parent.width_m();
        let pd = dem(parent, |u, _| 2800.0 + (u * pw * 1.2) as f32);
        let (a, b) = (Amplifier::new(t, &pd), Amplifier::new(right, &pd));
        for k in 0..10 {
            let v = k as f64 / 9.0;
            let (h1, h2) = (a.delta(1.0, v, 3200.0, 5.0), b.delta(0.0, v, 3200.0, 5.0));
            assert!((h1 - h2).abs() < 1e-3, "{h1} {h2}");
        }
        let mut g = TerrainGrid::from_heights(t, &pd, 32);
        let before = g.heights.clone();
        apply(&mut g, &a);
        assert!(g.heights.iter().zip(&before).any(|(x, y)| (x - y).abs() > 0.5));
    }
}
