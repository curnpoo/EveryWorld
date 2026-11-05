//! Terrain horizon maps: unlimited-distance terrain sun shadows
//! (`docs/plans/shadows.md`, stage 2 prototype).
//!
//! For each point of a coarse grid and each of [`AZIMUTHS`] compass
//! directions we store the elevation angle of the highest terrain seen in
//! that direction (out to `reach_m`, with Earth-curvature drop). The sun does
//! not enter the bake, so a tile bakes once on its worker thread and stays
//! valid all day. At shading time:
//!
//! `lit = smoothstep(-w, w, sun_elevation - horizon(sun_azimuth))`
//!
//! which shadows valleys behind ridges tens of kilometres away, at any camera
//! distance, for ~16 bytes per grid point. It only knows about terrain:
//! buildings and trees still need cascades (near) or a canopy heightfield (far).
//!
//! Angles are stored as `u8` on a square-root curve: low sun (where shadows
//! are long and errors show) gets the precision; 0.35° steps near the horizon.

use std::f64::consts::{FRAC_PI_2, TAU};

/// Compass directions per grid point (N, NNE, NE, ... clockwise from north).
pub const AZIMUTHS: usize = 16;
/// Mean Earth radius for the curvature drop.
const EARTH_R: f64 = 6_371_000.0;

/// Baked horizon angles for an `n x n` grid spanning `extent_m` x `extent_m`
/// (row 0 = north edge, like `TerrainGrid`).
#[derive(Clone, Debug, PartialEq)]
pub struct HorizonMap {
    pub n: usize,
    pub extent_m: f64,
    /// `n * n * AZIMUTHS` encoded angles, point-major.
    pub data: Vec<u8>,
}

/// Bake settings. `reach_m` is how far a ridge can cast; `first_step_m` the
/// first march step (grow geometrically by `growth`).
#[derive(Clone, Copy, Debug)]
pub struct BakeParams {
    pub n: usize,
    pub extent_m: f64,
    pub reach_m: f64,
    pub first_step_m: f64,
    pub growth: f64,
}

impl BakeParams {
    /// A z13-ish tile (~4.9 km at mid latitudes): 33x33 points (150 m), 30 km reach.
    pub fn tile(extent_m: f64) -> Self {
        BakeParams { n: 33, extent_m, reach_m: 30_000.0, first_step_m: extent_m / 64.0, growth: 1.12 }
    }
}

/// Encode an elevation angle (radians) to u8 on a sqrt curve; below 0 clamps to 0.
pub fn encode(angle: f64) -> u8 {
    let a = (angle / FRAC_PI_2).clamp(0.0, 1.0);
    (a.sqrt() * 255.0).round() as u8
}

/// Inverse of [`encode`].
pub fn decode(b: u8) -> f64 {
    let s = b as f64 / 255.0;
    s * s * FRAC_PI_2
}

/// Elevation angle of the terrain horizon from `(x, y)` (east, north metres)
/// towards compass azimuth `az` (radians, 0 = north, clockwise).
pub fn horizon_angle(height: &impl Fn(f64, f64) -> f64, x: f64, y: f64, az: f64, p: &BakeParams) -> f64 {
    let h0 = height(x, y);
    let (dx, dy) = (az.sin(), az.cos());
    let mut best = f64::NEG_INFINITY;
    let mut t = p.first_step_m;
    let mut step = p.first_step_m;
    while t <= p.reach_m {
        let h = height(x + dx * t, y + dy * t) - t * t / (2.0 * EARTH_R);
        best = best.max(((h - h0) / t).atan());
        step *= p.growth;
        t += step;
    }
    best
}

impl HorizonMap {
    /// Bake from a height function over local east/north metres, where the
    /// grid's north-west corner is `(0, extent)` and south-east is `(extent, 0)`.
    /// The function must cover `reach_m` around the tile (neighbour tiles or a
    /// coarser elevation zoom); off-data it should return a sensible fallback.
    pub fn bake(height: impl Fn(f64, f64) -> f64, p: &BakeParams) -> Self {
        let n = p.n.max(2);
        let mut data = Vec::with_capacity(n * n * AZIMUTHS);
        for row in 0..n {
            let y = p.extent_m * (1.0 - row as f64 / (n - 1) as f64);
            for col in 0..n {
                let x = p.extent_m * col as f64 / (n - 1) as f64;
                for k in 0..AZIMUTHS {
                    let az = k as f64 * TAU / AZIMUTHS as f64;
                    data.push(encode(horizon_angle(&height, x, y, az, p)));
                }
            }
        }
        HorizonMap { n, extent_m: p.extent_m, data }
    }

    fn at(&self, row: usize, col: usize, k: usize) -> f64 {
        decode(self.data[(row * self.n + col) * AZIMUTHS + k % AZIMUTHS])
    }

    /// Horizon elevation (radians) at tile `u` (west->east) `v` (north->south)
    /// in [0,1], towards compass azimuth `az`: bilinear in space, linear in azimuth.
    /// This is what the terrain fragment shader evaluates per pixel.
    pub fn horizon(&self, u: f64, v: f64, az: f64) -> f64 {
        let f = (self.n - 1) as f64;
        let (gx, gy) = (u.clamp(0.0, 1.0) * f, v.clamp(0.0, 1.0) * f);
        let (c0, r0) = (gx.floor().min(f - 1.0) as usize, gy.floor().min(f - 1.0) as usize);
        let (fx, fy) = (gx - c0 as f64, gy - r0 as f64);
        let a = az.rem_euclid(TAU) / TAU * AZIMUTHS as f64;
        let k0 = a.floor() as usize;
        let fa = a - k0 as f64;
        let s = |r: usize, c: usize| self.at(r, c, k0) * (1.0 - fa) + self.at(r, c, k0 + 1) * fa;
        let top = s(r0, c0) * (1.0 - fx) + s(r0, c0 + 1) * fx;
        let bot = s(r0 + 1, c0) * (1.0 - fx) + s(r0 + 1, c0 + 1) * fx;
        top * (1.0 - fy) + bot * fy
    }

    /// Sun visibility in [0,1] (1 = lit). `soft` is the half-width of the
    /// penumbra in radians (the sun's disc is ~0.0047 rad; wider hides the
    /// coarse grid).
    pub fn visibility(&self, u: f64, v: f64, sun_az: f64, sun_elev: f64, soft: f64) -> f64 {
        let x = ((sun_elev - self.horizon(u, v, sun_az)) / soft).clamp(-1.0, 1.0) * 0.5 + 0.5;
        x * x * (3.0 - 2.0 * x)
    }

    /// Bytes per tile (for the residency budget).
    pub fn bytes(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXT: f64 = 4_000.0;

    fn params() -> BakeParams {
        BakeParams { n: 9, extent_m: EXT, reach_m: 30_000.0, first_step_m: 40.0, growth: 1.1 }
    }
    fn deg(d: f64) -> f64 {
        d.to_radians()
    }

    #[test]
    fn encoding_is_precise_at_low_sun() {
        for d in [0.0, 0.5, 1.0, 3.0, 8.0, 20.0, 45.0, 89.0] {
            let err = (decode(encode(deg(d))) - deg(d)).abs().to_degrees();
            let tol = if d <= 10.0 { 0.4 } else { 1.0 };
            assert!(err < tol, "{d} deg -> err {err}");
        }
        assert_eq!(encode(-0.3), 0);
        assert_eq!(encode(2.0), 255);
    }

    #[test]
    fn flat_ground_has_no_horizon_and_is_lit() {
        let m = HorizonMap::bake(|_, _| 500.0, &params());
        // Curvature only lowers the horizon: everything clamps to 0.
        assert!(m.data.iter().all(|&b| b == 0));
        assert!(m.visibility(0.5, 0.5, deg(200.0), deg(1.0), deg(0.5)) > 0.99);
    }

    #[test]
    fn ridge_to_the_east_shadows_low_morning_sun_only() {
        // A 1000 m ridge 6 km east of the tile's west edge (2 km beyond the tile).
        let ridge = |x: f64, _y: f64| if (x - 6_000.0).abs() < 300.0 { 1_000.0 } else { 0.0 };
        let m = HorizonMap::bake(ridge, &params());
        // From the tile centre (x=2000) the ridge crest is ~3.7 km away: atan(1000/3700) ~ 15 deg.
        let h_east = m.horizon(0.5, 0.5, deg(90.0)).to_degrees();
        assert!((13.0..17.5).contains(&h_east), "east horizon {h_east}");
        assert!(m.horizon(0.5, 0.5, deg(270.0)) < deg(0.5));
        // Morning sun low in the east: shadowed. Evening sun low in the west: lit.
        assert!(m.visibility(0.5, 0.5, deg(90.0), deg(8.0), deg(0.5)) < 0.01);
        assert!(m.visibility(0.5, 0.5, deg(270.0), deg(8.0), deg(0.5)) > 0.99);
        // High sun clears the ridge.
        assert!(m.visibility(0.5, 0.5, deg(90.0), deg(30.0), deg(0.5)) > 0.99);
        // The west edge is further from the ridge, so its horizon is lower.
        assert!(m.horizon(0.0, 0.5, deg(90.0)) < m.horizon(1.0, 0.5, deg(90.0)));
    }

    #[test]
    fn earth_curvature_hides_distant_low_hills() {
        // A 6 km wide, 100 m plateau 22-28 km away: flat-Earth angle 0.26 deg,
        // curvature drop (22 km)^2 / 2R = 38 m cuts it to ~0.16. A 30 m one
        // disappears entirely. (Steps are ~2 km out here, so far sources must be
        // pre-filtered to that scale, e.g. a coarse/max-mip elevation zoom.)
        let far = |h: f64| move |x: f64, _y: f64| if (x - 27_000.0).abs() < 3_000.0 { h } else { 0.0 };
        let p = BakeParams { reach_m: 30_000.0, ..params() };
        let a100 = horizon_angle(&far(100.0), 2_000.0, 2_000.0, deg(90.0), &p);
        let a30 = horizon_angle(&far(30.0), 2_000.0, 2_000.0, deg(90.0), &p);
        assert!(a100 > 0.0 && a100.to_degrees() < 0.2, "{}", a100.to_degrees());
        assert!(a30 < 0.0);
    }

    #[test]
    fn azimuth_interpolation_wraps_through_north() {
        let north_wall = |_x: f64, y: f64| if y > 5_000.0 { 800.0 } else { 0.0 };
        let m = HorizonMap::bake(north_wall, &params());
        let a = m.horizon(0.5, 0.5, deg(359.0));
        let b = m.horizon(0.5, 0.5, deg(1.0));
        assert!(a > deg(10.0) && (a - b).abs() < deg(1.0), "{} {}", a.to_degrees(), b.to_degrees());
    }

    #[test]
    fn bake_is_deterministic_and_sized() {
        let hill = |x: f64, y: f64| 300.0 * (-((x - 1_500.0).powi(2) + (y - 2_500.0).powi(2)) / 1.0e6).exp();
        let a = HorizonMap::bake(hill, &params());
        let b = HorizonMap::bake(hill, &params());
        assert_eq!(a, b);
        assert_eq!(a.bytes(), 9 * 9 * AZIMUTHS);
        assert_eq!(BakeParams::tile(4_900.0).n * BakeParams::tile(4_900.0).n * AZIMUTHS, 17_424);
    }
}
