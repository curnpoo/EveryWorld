//! Far sun shadows: what the terrain shader uses past the last cascade
//! (`docs/plans/shadows.md`, stage 2). Opt-in with `EW_FAR_SHADOWS=1`.
//!
//! Each tile bakes two small textures on its worker thread. Neither depends
//! on the sun, so a tile bakes once and stays valid all day:
//!
//! * **Horizon map** ([`crate::horizon`]): per grid point, the elevation angle
//!   of the highest terrain in 16 compass directions, out to 30-150 km, with
//!   Earth curvature. Heights past the tile edge come from a coarser elevation
//!   zoom (3x3 tiles around the tile's ancestor, LRU-cached). This is what makes
//!   valleys go dark behind Everest when you look from orbit.
//! * **Surface map** (128x128 RGBA f16): terrain height, canopy density and
//!   building heights. The shader uses it for canopy darkening (a forest is
//!   darker than a meadow under a low sun) and a short height-field ray march
//!   toward the sun (forest edges and tall buildings cast onto nearby ground).
//!
//! We compute our own shadows from geometry. Shadows already baked into the
//! satellite imagery are not used.

use crate::horizon::{AZIMUTHS, BakeParams, HorizonMap};
use crate::project::TileProjector;
use crate::surface::SurfaceMap;
use crate::terrain::TerrainGrid;
use ed_data::DiskCache;
use ed_data::elevation::{self, HeightTile};
use ed_geo::tile::TileId;
use glam::{DVec3, Vec3};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Turn far-shadow baking on (the game sets this from `EW_FAR_SHADOWS=1`).
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Horizon texture: 4 azimuth groups side by side, `HZ_N` points each, plus
/// one metadata row. Must match `terrain.wesl`.
pub const HZ_N: usize = 65;
pub const HZ_W: u32 = (AZIMUTHS / 4 * HZ_N) as u32;
pub const HZ_H: u32 = HZ_N as u32 + 1;
/// Surface texture side (texels).
pub const SURF_N: usize = 128;
/// Height of a closed canopy (m): what a 100 % canopy cell adds to the surface.
pub const CANOPY_H: f32 = 14.0;
/// Decoded coarse elevation tiles kept for neighbour lookups.
const COARSE_CAP: usize = 64;

/// One tile's far-shadow textures, ready to upload.
#[derive(Clone, Debug, PartialEq)]
pub struct FarShadowTex {
    /// `HZ_W x HZ_H` RGBA8.
    pub horizon: Vec<u8>,
    /// `SURF_N x SURF_N` RGBA16F as raw f16 bits: R = surface top above the
    /// tile's lowest point, G = terrain, B = canopy density 0..1, A = building height.
    pub surface: Vec<u16>,
}

impl FarShadowTex {
    pub fn bytes(&self) -> usize {
        self.horizon.len() + self.surface.len() * 2
    }
}

/// Bake settings for a tile `extent_m` wide at zoom `z`.
pub fn horizon_params(z: u8, extent_m: f64) -> BakeParams {
    let n = if z <= 11 { HZ_N } else { 33 };
    BakeParams {
        n,
        extent_m,
        reach_m: (0.5 * extent_m).clamp(30_000.0, 150_000.0),
        first_step_m: extent_m / (2.0 * (n - 1) as f64),
        growth: 1.12,
    }
}

/// Elevation zoom used for heights beyond the tile: the finest one whose
/// tiles are at least `reach_m` wide, so a 3x3 block covers the reach.
pub fn coarse_zoom(tile: TileId, reach_m: f64) -> u8 {
    let top = tile.z.min(13);
    (0..=top).rev().find(|&z| tile.ancestor(z).0.width_m() >= reach_m).unwrap_or(0)
}

/// The 3x3 coarse tiles to prefetch on the IO thread.
pub fn coarse_tiles(tile: TileId) -> Vec<TileId> {
    let p = horizon_params(tile.z, tile.width_m());
    let a = tile.ancestor(coarse_zoom(tile, p.reach_m)).0;
    (-1..=1).flat_map(|dy| (-1..=1).filter_map(move |dx| a.offset(dx, dy))).collect()
}

pub fn prefetch(cache: &DiskCache, tile: TileId) {
    for t in coarse_tiles(tile) {
        let _ = elevation::prefetch(cache, t);
    }
}

type Lru = Mutex<(u64, HashMap<TileId, (Option<Arc<HeightTile>>, u64)>)>;

fn lru() -> &'static Lru {
    static L: OnceLock<Lru> = OnceLock::new();
    L.get_or_init(|| Mutex::new((0, HashMap::new())))
}

/// A decoded coarse elevation tile, shared between neighbouring tiles' bakes.
/// Failures are remembered too, so a missing tile is not refetched per bake.
fn coarse_tile(cache: &DiskCache, id: TileId) -> Option<Arc<HeightTile>> {
    {
        let mut g = lru().lock().unwrap();
        g.0 += 1;
        let tick = g.0;
        if let Some(e) = g.1.get_mut(&id) {
            e.1 = tick;
            return e.0.clone();
        }
    }
    let t = elevation::fetch(cache, id).ok().map(Arc::new);
    let mut g = lru().lock().unwrap();
    let tick = g.0;
    g.1.insert(id, (t.clone(), tick));
    if g.1.len() > COARSE_CAP
        && let Some(old) = g.1.iter().min_by_key(|e| e.1.1).map(|e| *e.0)
    {
        g.1.remove(&old);
    }
    t
}

/// Heights around a tile: its own grid inside, a 3x3 block of coarser tiles
/// outside (mercator-linear mapping; missing neighbours clamp to the edge).
pub struct Neighbourhood {
    /// Tile position inside the 3x3 block, in coarse-tile units.
    origin: (f64, f64),
    scale: f64,
    tiles: [Option<Arc<HeightTile>>; 9],
}

impl Neighbourhood {
    pub fn load(cache: &DiskCache, tile: TileId, reach_m: f64) -> Self {
        let zs = coarse_zoom(tile, reach_m);
        let (a, u0, v0, s) = tile.ancestor(zs);
        let mut tiles: [Option<Arc<HeightTile>>; 9] = Default::default();
        for dy in -1..=1i64 {
            for dx in -1..=1i64 {
                tiles[((dy + 1) * 3 + dx + 1) as usize] = a.offset(dx, dy).and_then(|t| coarse_tile(cache, t));
            }
        }
        Neighbourhood { origin: (1.0 + u0, 1.0 + v0), scale: s, tiles }
    }

    /// Height at tile `(u, v)` outside [0,1]^2, if a coarse tile covers it.
    pub fn sample(&self, u: f64, v: f64) -> Option<f32> {
        let gx = self.origin.0 + u * self.scale;
        let gy = self.origin.1 + v * self.scale;
        if !(0.0..3.0).contains(&gx) || !(0.0..3.0).contains(&gy) {
            return None;
        }
        let (ix, iy) = (gx.floor(), gy.floor());
        let t = self.tiles[iy as usize * 3 + ix as usize].as_ref()?;
        Some(t.sample(gx - ix, gy - iy))
    }
}

/// Bake the horizon map for a tile from its grid plus neighbourhood.
pub fn bake_horizon(grid: &TerrainGrid, nb: Option<&Neighbourhood>, p: &BakeParams) -> HorizonMap {
    let ext = p.extent_m;
    let height = |x: f64, y: f64| {
        let (u, v) = (x / ext, 1.0 - y / ext);
        let inside = (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v);
        let h = if inside { None } else { nb.and_then(|n| n.sample(u, v)) };
        h.unwrap_or_else(|| grid.sample(u, v)) as f64
    };
    HorizonMap::bake(height, p)
}

/// Pack a horizon map into the `HZ_W x HZ_H` RGBA8 layout `terrain.wesl` reads:
/// texel `(g * HZ_N + col, row)` channel `c` holds azimuth `4g + c`; the last
/// row holds `extent_m` (f32 bits) at texel 0 and `n` (u32) at texel 1.
pub fn pack_horizon(m: &HorizonMap) -> Vec<u8> {
    assert!(m.n <= HZ_N && AZIMUTHS == 16);
    let w = HZ_W as usize;
    let mut out = vec![0u8; w * HZ_H as usize * 4];
    for row in 0..m.n {
        for col in 0..m.n {
            let src = (row * m.n + col) * AZIMUTHS;
            for g in 0..4 {
                let dst = (row * w + g * HZ_N + col) * 4;
                out[dst..dst + 4].copy_from_slice(&m.data[src + 4 * g..src + 4 * g + 4]);
            }
        }
    }
    let meta = HZ_N * w * 4;
    out[meta..meta + 4].copy_from_slice(&(m.extent_m as f32).to_le_bytes());
    out[meta + 4..meta + 8].copy_from_slice(&(m.n as u32).to_le_bytes());
    out
}

/// f32 -> IEEE half bits (round to nearest; overflow saturates to the max
/// finite value, tiny values flush to zero). Enough for heights and densities.
pub fn f16_bits(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let a = x.abs();
    if a.is_nan() {
        return 0x7E00;
    }
    if a >= 65504.0 {
        return sign | 0x7BFF;
    }
    if a < 6.1e-5 {
        return sign;
    }
    let exp = ((b >> 23) & 0xFF) as i32 - 127 + 15;
    let mant = b & 0x7F_FFFF;
    let mut h = ((exp as u32) << 10) | (mant >> 13);
    // Round half up on the dropped bits (carry into the exponent is correct).
    if mant & 0x1000 != 0 {
        h += 1;
    }
    sign | h.min(0x7BFF) as u16
}

/// Inverse of [`f16_bits`] for normal values (tests, debugging).
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = ((h >> 10) & 0x1F) as i32;
    let mant = (h & 0x3FF) as f32;
    if exp == 0 {
        return sign * mant * 2f32.powi(-24);
    }
    sign * (1.0 + mant / 1024.0) * 2f32.powi(exp - 15)
}

/// A building footprint in tile uv with its height above its base.
pub struct UvFootprint {
    pub ring: Vec<(f32, f32)>,
    pub height: f32,
}

/// Maps tile-local render positions back to tile uv (least squares on the
/// plane spanned by the tile's u and v edges; exact enough over one tile).
pub struct UvMapper {
    p00: Vec3,
    inv: [[f32; 3]; 2],
}

impl UvMapper {
    pub fn new(proj: &TileProjector) -> Self {
        let p00 = proj.local(0.0, 0.0, 0.0);
        let a = proj.local(1.0, 0.0, 0.0) - p00;
        let b = proj.local(0.0, 1.0, 0.0) - p00;
        // (M^T M)^-1 M^T with M = [a b].
        let (aa, ab, bb) = (a.dot(a), a.dot(b), b.dot(b));
        let det = aa * bb - ab * ab;
        let ra = (a * bb - b * ab) / det;
        let rb = (b * aa - a * ab) / det;
        UvMapper { p00, inv: [ra.to_array(), rb.to_array()] }
    }

    pub fn uv(&self, p: Vec3) -> (f32, f32) {
        let d = p - self.p00;
        (Vec3::from(self.inv[0]).dot(d), Vec3::from(self.inv[1]).dot(d))
    }
}

fn inside(ring: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut c = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (xi, yi) = ring[i];
        let (xj, yj) = ring[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            c = !c;
        }
        j = i;
    }
    c
}

/// Max building height per texel (2x2 subsamples, so small houses still register).
pub fn raster_buildings(n: usize, foot: &[UvFootprint]) -> Vec<f32> {
    let mut out = vec![0.0f32; n * n];
    let nf = n as f32;
    for f in foot.iter().filter(|f| f.ring.len() >= 3 && f.height > 0.0) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(u, v) in &f.ring {
            (x0, y0, x1, y1) = (x0.min(u), y0.min(v), x1.max(u), y1.max(v));
        }
        // Footprints smaller than a texel can miss every subsample: always
        // stamp the texel under the vertex centroid.
        let k = f.ring.len() as f32;
        let (mx, my) = f.ring.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0 / k, a.1 + p.1 / k));
        if (0.0..1.0).contains(&mx) && (0.0..1.0).contains(&my) {
            let o = &mut out[(my * nf) as usize * n + (mx * nf) as usize];
            *o = o.max(f.height);
        }
        let cx0 = ((x0 * nf).floor().max(0.0)) as usize;
        let cy0 = ((y0 * nf).floor().max(0.0)) as usize;
        let cx1 = ((x1 * nf).ceil().min(nf)) as usize;
        let cy1 = ((y1 * nf).ceil().min(nf)) as usize;
        for ty in cy0..cy1 {
            for tx in cx0..cx1 {
                let hit = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)]
                    .iter()
                    .any(|&(sx, sy)| inside(&f.ring, (tx as f32 + sx) / nf, (ty as f32 + sy) / nf));
                if hit {
                    let o = &mut out[ty * n + tx];
                    *o = o.max(f.height);
                }
            }
        }
    }
    out
}

/// Pack the surface texture from per-texel samplers (texel centres in uv).
pub fn bake_surface(
    n: usize,
    terrain: impl Fn(f64, f64) -> f32,
    canopy: impl Fn(f64, f64) -> f32,
    buildings: &[f32],
) -> Vec<u16> {
    let mut out = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let (u, v) = ((x as f64 + 0.5) / n as f64, (y as f64 + 0.5) / n as f64);
            let g = terrain(u, v);
            let d = canopy(u, v).clamp(0.0, 1.0);
            let b = buildings[y * n + x];
            let top = g + b.max(CANOPY_H * d);
            out.extend([f16_bits(top), f16_bits(g), f16_bits(d), f16_bits(b)]);
        }
    }
    out
}

/// Everything a tile build needs to hand over.
pub struct TileInputs<'a> {
    pub tile: TileId,
    pub grid: &'a TerrainGrid,
    pub hmin: f32,
    pub surface: &'a SurfaceMap,
    pub proj: &'a TileProjector,
    /// Building outlines (tile-local render positions) and heights.
    pub buildings: Vec<(Vec<Vec3>, f32)>,
}

/// Bake both textures for a tile (worker thread).
pub fn bake(cache: &DiskCache, inp: &TileInputs) -> FarShadowTex {
    let p = horizon_params(inp.tile.z, inp.tile.width_m());
    let nb = Neighbourhood::load(cache, inp.tile, p.reach_m);
    let horizon = pack_horizon(&bake_horizon(inp.grid, Some(&nb), &p));
    let map = UvMapper::new(inp.proj);
    let foot: Vec<UvFootprint> = inp
        .buildings
        .iter()
        .map(|(ring, h)| UvFootprint { ring: ring.iter().map(|&q| map.uv(q)).collect(), height: *h })
        .collect();
    let bl = raster_buildings(SURF_N, &foot);
    let surface = bake_surface(
        SURF_N,
        |u, v| inp.grid.sample(u, v) - inp.hmin,
        |u, v| inp.surface.canopy_share(u, v),
        &bl,
    );
    FarShadowTex { horizon, surface }
}

/// Earth centre and pole axis in a frame's render space (for the shader's
/// per-fragment local up / north).
pub fn earth_axes(ecef_to_render: glam::DAffine3) -> (DVec3, DVec3) {
    (ecef_to_render.transform_point3(DVec3::ZERO), ecef_to_render.transform_vector3(DVec3::Z).normalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f16_round_trips_heights_and_densities() {
        for x in [0.0f32, 0.25, 0.5, 1.0, 14.0, 123.4, 1_500.0, 8_848.0] {
            let y = f16_to_f32(f16_bits(x));
            assert!((y - x).abs() <= x.abs() / 1024.0 + 1e-6, "{x} -> {y}");
        }
        assert_eq!(f16_bits(1.0), 0x3C00);
        assert_eq!(f16_bits(-2.0), 0xC000);
        assert_eq!(f16_bits(1e9), 0x7BFF);
    }

    #[test]
    fn horizon_packing_matches_shader_layout() {
        let n = 3;
        let data: Vec<u8> = (0..n * n * AZIMUTHS).map(|i| (i % 251) as u8).collect();
        let m = HorizonMap { n, extent_m: 4_321.5, data: data.clone() };
        let px = pack_horizon(&m);
        assert_eq!(px.len(), (HZ_W * HZ_H * 4) as usize);
        // Point (row 2, col 1), azimuth 9 = group 2, channel 1.
        let (row, col, k) = (2, 1, 9);
        let t = (row * HZ_W as usize + (k / 4) * HZ_N + col) * 4 + k % 4;
        assert_eq!(px[t], data[(row * n + col) * AZIMUTHS + k]);
        let meta = HZ_N * HZ_W as usize * 4;
        assert_eq!(f32::from_le_bytes(px[meta..meta + 4].try_into().unwrap()), 4_321.5);
        assert_eq!(u32::from_le_bytes(px[meta + 4..meta + 8].try_into().unwrap()), n as u32);
    }

    #[test]
    fn coarse_zoom_covers_the_reach() {
        for z in [6u8, 8, 10, 12, 14] {
            let t = TileId::containing(27.99, 86.93, z);
            let p = horizon_params(z, t.width_m());
            let zs = coarse_zoom(t, p.reach_m);
            assert!(zs <= z.min(13));
            assert!(t.ancestor(zs).0.width_m() >= p.reach_m || zs == 0, "z{z} -> z{zs}");
            assert!((30_000.0..=150_000.0).contains(&p.reach_m));
            assert_eq!(coarse_tiles(t).len(), 9);
        }
    }

    #[test]
    fn buildings_raster_inside_their_footprint() {
        let sq = |x0: f32, y0: f32, s: f32| vec![(x0, y0), (x0 + s, y0), (x0 + s, y0 + s), (x0, y0 + s)];
        let foot = [
            UvFootprint { ring: sq(0.1, 0.1, 0.2), height: 20.0 },
            UvFootprint { ring: sq(0.15, 0.15, 0.05), height: 45.0 },
            // A house smaller than a texel still shows up.
            UvFootprint { ring: sq(0.703, 0.703, 0.004), height: 8.0 },
        ];
        let n = 32;
        let r = raster_buildings(n, &foot);
        let at = |u: f32, v: f32| r[(v * n as f32) as usize * n + (u * n as f32) as usize];
        assert_eq!(at(0.2, 0.25), 20.0);
        assert_eq!(at(0.17, 0.17), 45.0);
        assert_eq!(at(0.5, 0.5), 0.0);
        assert_eq!(at(0.705, 0.705), 8.0);
    }

    #[test]
    fn surface_top_is_terrain_plus_tallest_cover() {
        let n = 4;
        let mut bl = vec![0.0; n * n];
        bl[5] = 30.0;
        let s = bake_surface(n, |u, _| 100.0 * u as f32, |_, v| if v < 0.5 { 1.0 } else { 0.0 }, &bl);
        let px = |i: usize| [0, 1, 2, 3].map(|c| f16_to_f32(s[i * 4 + c]));
        // Texel 0 (forest): top = ground + canopy.
        let [top, g, d, b] = px(0);
        assert!((top - (g + CANOPY_H)).abs() < 0.1 && d == 1.0 && b == 0.0);
        // Texel 5 (forest + 30 m building): building wins.
        let [top, g, _, b] = px(5);
        assert!((top - (g + 30.0)).abs() < 0.2 && b == 30.0);
        // Texel 12 (open ground).
        let [top, g, d, _] = px(12);
        assert!((top - g).abs() < 0.05 && d == 0.0);
    }

    /// `cargo test --release -p ed_world bake_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bake_cost() {
        let hills = |x: f64, y: f64| 800.0 * ((x / 3_000.0).sin() * (y / 4_100.0).cos()) + 1_000.0;
        for (z, w) in [(8u8, 110_000.0), (11, 13_800.0), (12, 6_900.0), (14, 1_730.0)] {
            let p = horizon_params(z, w);
            let t = std::time::Instant::now();
            let m = HorizonMap::bake(hills, &p);
            eprintln!("z{z}: n={} reach={:.0} km  {:?}  ({} B)", p.n, p.reach_m / 1e3, t.elapsed(), m.bytes());
        }
    }

    #[test]
    fn uv_mapper_inverts_the_projector() {
        let tile = TileId::containing(46.02, 7.75, 14);
        let frame = ed_geo::EnuFrame::new(tile.center());
        let proj = TileProjector::new(tile, frame);
        let m = UvMapper::new(&proj);
        for (u, v) in [(0.1, 0.2), (0.5, 0.5), (0.9, 0.7)] {
            let (a, b) = m.uv(proj.local(u, v, 1_600.0));
            assert!((a as f64 - u).abs() < 2e-3 && (b as f64 - v).abs() < 2e-3, "{u},{v} -> {a},{b}");
        }
    }
}
