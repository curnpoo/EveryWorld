//! What a fine tile sounds like, as data: a coarse grid of distances to
//! sound sources (big roads, rail, sea, lakes, rivers, waterfalls) and how
//! built-up each cell is (street-canyon echo, city hum). Built on the tile
//! worker from the same MVT and `TerrainGrid` as everything else; the game
//! samples it under the camera and feeds `ed_audio::Params`.
//!
//! Distances come from a two-pass chamfer transform, so they are accurate to
//! about half a cell (~20 m at z14): plenty for ambience levels.

use crate::terrain::TerrainGrid;
use ed_data::mvt::{self, GeomType};

pub const RES: usize = 64;

/// Source layers, indexes into `SoundMap::dist`.
pub mod src {
    pub const MOTORWAY: usize = 0;
    pub const ARTERIAL: usize = 1;
    pub const MINOR: usize = 2;
    pub const RAIL: usize = 3;
    pub const OCEAN: usize = 4;
    pub const LAKE: usize = 5;
    pub const RIVER: usize = 6;
    pub const WATERFALL: usize = 7;
    pub const N: usize = 8;
}

/// Far value (m) for "none in this tile".
pub const FAR_M: f32 = 5_000.0;

#[derive(Debug, Clone)]
pub struct SoundMap {
    pub cell_m: f32,
    /// Metres to the nearest source of each kind, per cell (row 0 = north).
    pub dist: [Vec<u16>; src::N],
    /// Building footprint share of the cell, 0..255.
    pub bldg_cover: Vec<u8>,
    /// Mean building height in the cell, metres.
    pub bldg_h: Vec<u8>,
    /// River steepness 0..255 where rivers run (rapids).
    pub river_steep: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SoundSample {
    pub dist: [f32; src::N],
    pub bldg_cover: f32,
    pub bldg_h: f32,
    pub river_steep: f32,
}

impl SoundMap {
    pub fn build(vt: &mvt::Tile, grid: &TerrainGrid, width_m: f32) -> SoundMap {
        let n = RES * RES;
        let cell_m = width_m / RES as f32;
        let mut seeds: [Vec<bool>; src::N] = std::array::from_fn(|_| vec![false; n]);
        let mut steep = vec![0u8; n];
        let cell = |u: f32, v: f32| -> Option<usize> {
            let (x, y) = ((u * RES as f32).floor() as i64, (v * RES as f32).floor() as i64);
            ((0..RES as i64).contains(&x) && (0..RES as i64).contains(&y)).then(|| y as usize * RES + x as usize)
        };
        // Walk a line in steps of ~half a cell, calling `f(u, v)` (tile uv).
        let walk = |line: &[[f32; 2]], ext: f32, f: &mut dyn FnMut(f32, f32)| {
            for w in line.windows(2) {
                let (a, b) = ([w[0][0] / ext, w[0][1] / ext], [w[1][0] / ext, w[1][1] / ext]);
                let len = ((b[0] - a[0]).abs().max((b[1] - a[1]).abs()) * RES as f32 * 2.0).ceil() as usize + 1;
                for k in 0..=len {
                    let t = k as f32 / len as f32;
                    f(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t);
                }
            }
        };

        if let Some(l) = vt.layer("transportation") {
            let ext = l.extent as f32;
            for f in l.features.iter().filter(|f| f.geom_type == GeomType::LineString) {
                if f.str("brunnel") == Some("tunnel") {
                    continue;
                }
                let k = match f.str("class").unwrap_or("") {
                    "motorway" | "trunk" => src::MOTORWAY,
                    "primary" | "secondary" => src::ARTERIAL,
                    "tertiary" | "minor" => src::MINOR,
                    "rail" => src::RAIL,
                    _ => continue,
                };
                for line in &f.rings {
                    walk(line, ext, &mut |u, v| {
                        if let Some(i) = cell(u, v) {
                            seeds[k][i] = true;
                        }
                    });
                }
            }
        }
        if let Some(l) = vt.layer("waterway") {
            let ext = l.extent as f32;
            for f in l.features.iter().filter(|f| f.geom_type == GeomType::LineString) {
                let big = match f.str("class").unwrap_or("") {
                    "river" | "canal" => true,
                    "stream" => false,
                    _ => continue,
                };
                for line in &f.rings {
                    // Steepness along the line from the terrain; a sharp drop is a waterfall.
                    let mut prev: Option<(f32, f32, f32)> = None;
                    walk(line, ext, &mut |u, v| {
                        let h = grid.sample(u as f64, v as f64);
                        let Some(i) = cell(u, v) else { prev = None; return };
                        seeds[src::RIVER][i] |= big || prev.is_some();
                        if let Some((pu, pv, ph)) = prev {
                            let d = ((u - pu).hypot(v - pv) * width_m).max(1.0);
                            if d > 8.0 {
                                let grade = (ph - h).abs() / d;
                                steep[i] = steep[i].max((grade / 0.12 * 255.0).min(255.0) as u8);
                                if grade > 0.45 && (ph - h).abs() > 6.0 {
                                    seeds[src::WATERFALL][i] = true;
                                }
                                prev = Some((u, v, h));
                            }
                        } else {
                            prev = Some((u, v, h));
                        }
                    });
                }
            }
        }
        if let Some(l) = vt.layer("water") {
            let ext = l.extent as f32;
            for f in l.features.iter().filter(|f| f.geom_type == GeomType::Polygon) {
                let k = if f.str("class") == Some("ocean") { src::OCEAN } else { src::LAKE };
                fill_polygon(&f.rings, ext, &mut |i| seeds[k][i] = true);
            }
        }

        let mut cover = vec![0u16; n];
        let mut hsum = vec![0f32; n];
        if let Some(l) = vt.layer("building") {
            let ext = l.extent as f32;
            for f in l.features.iter().filter(|f| f.geom_type == GeomType::Polygon) {
                let h = f.num("render_height").unwrap_or(8.0) as f32;
                let mut hits = Vec::new();
                fill_polygon_sub(&f.rings, ext, 2, &mut |i| hits.push(i));
                for i in hits {
                    cover[i] += 1;
                    hsum[i] += h;
                }
            }
        }

        let dist = std::array::from_fn(|k| chamfer(&seeds[k], cell_m));
        SoundMap {
            cell_m,
            dist,
            bldg_cover: cover.iter().map(|c| (*c as f32 / 4.0 * 255.0).min(255.0) as u8).collect(),
            bldg_h: cover.iter().zip(&hsum).map(|(c, h)| if *c > 0 { (h / *c as f32).min(255.0) as u8 } else { 0 }).collect(),
            river_steep: steep,
        }
    }

    /// Bilinear sample at tile uv.
    pub fn sample(&self, u: f32, v: f32) -> SoundSample {
        let fx = (u * RES as f32 - 0.5).clamp(0.0, (RES - 1) as f32);
        let fy = (v * RES as f32 - 0.5).clamp(0.0, (RES - 1) as f32);
        let (x0, y0) = (fx as usize, fy as usize);
        let (x1, y1) = ((x0 + 1).min(RES - 1), (y0 + 1).min(RES - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let bl = |get: &dyn Fn(usize) -> f32| {
            let a = get(y0 * RES + x0) * (1.0 - tx) + get(y0 * RES + x1) * tx;
            let b = get(y1 * RES + x0) * (1.0 - tx) + get(y1 * RES + x1) * tx;
            a * (1.0 - ty) + b * ty
        };
        SoundSample {
            dist: std::array::from_fn(|k| bl(&|i| self.dist[k][i] as f32)),
            bldg_cover: bl(&|i| self.bldg_cover[i] as f32 / 255.0),
            bldg_h: bl(&|i| self.bldg_h[i] as f32),
            river_steep: bl(&|i| self.river_steep[i] as f32 / 255.0),
        }
    }
}

/// Even-odd point-in-polygon over all rings (holes included) at cell centres.
fn fill_polygon(rings: &[Vec<[f32; 2]>], ext: f32, f: &mut dyn FnMut(usize)) {
    fill_polygon_sub(rings, ext, 1, f)
}

/// Like `fill_polygon` with `sub`² samples per cell; calls `f` once per hit.
fn fill_polygon_sub(rings: &[Vec<[f32; 2]>], ext: f32, sub: usize, f: &mut dyn FnMut(usize)) {
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for r in rings {
        for p in r {
            for a in 0..2 {
                lo[a] = lo[a].min(p[a] / ext);
                hi[a] = hi[a].max(p[a] / ext);
            }
        }
    }
    let g = (RES * sub) as f32;
    let span = |a: usize| -> (usize, usize) {
        let s = ((lo[a] * g - 0.5).floor().max(0.0)) as usize;
        let e = ((hi[a] * g - 0.5).ceil().min(g - 1.0)).max(0.0) as usize;
        (s, e)
    };
    let ((x0, x1), (y0, y1)) = (span(0), span(1));
    if lo[0] >= 1.0 || lo[1] >= 1.0 || hi[0] < 0.0 || hi[1] < 0.0 {
        return;
    }
    for y in y0..=y1 {
        let py = (y as f32 + 0.5) / g * ext;
        for x in x0..=x1 {
            let px = (x as f32 + 0.5) / g * ext;
            let mut inside = false;
            for r in rings {
                let m = r.len();
                for i in 0..m {
                    let (a, b) = (r[i], r[(i + m - 1) % m]);
                    if (a[1] > py) != (b[1] > py) && px < (b[0] - a[0]) * (py - a[1]) / (b[1] - a[1]) + a[0] {
                        inside = !inside;
                    }
                }
            }
            if inside {
                f((y / sub) * RES + x / sub);
            }
        }
    }
}

/// Two-pass 3-4 chamfer distance transform, in metres, capped at `FAR_M`.
fn chamfer(seed: &[bool], cell_m: f32) -> Vec<u16> {
    let big = f32::MAX / 4.0;
    let mut d: Vec<f32> = seed.iter().map(|s| if *s { 0.0 } else { big }).collect();
    let (a, b) = (cell_m, cell_m * std::f32::consts::SQRT_2);
    let r = RES as i64;
    let at = |x: i64, y: i64| (y * r + x) as usize;
    for y in 0..r {
        for x in 0..r {
            let mut v = d[at(x, y)];
            for (dx, dy, w) in [(-1, 0, a), (-1, -1, b), (0, -1, a), (1, -1, b)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && nx < r && ny >= 0 {
                    v = v.min(d[at(nx, ny)] + w);
                }
            }
            d[at(x, y)] = v;
        }
    }
    for y in (0..r).rev() {
        for x in (0..r).rev() {
            let mut v = d[at(x, y)];
            for (dx, dy, w) in [(1, 0, a), (1, 1, b), (0, 1, a), (-1, 1, b)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && nx < r && ny < r {
                    v = v.min(d[at(nx, ny)] + w);
                }
            }
            d[at(x, y)] = v;
        }
    }
    d.iter().map(|v| v.min(FAR_M) as u16).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn line(class: &str, pts: Vec<[f32; 2]>) -> mvt::Feature {
        let mut props = HashMap::new();
        props.insert("class".to_string(), mvt::Value::String(class.into()));
        mvt::Feature { id: None, geom_type: GeomType::LineString, props, rings: vec![pts] }
    }

    fn flat_grid() -> TerrainGrid {
        TerrainGrid { tile: ed_geo::tile::TileId { z: 14, x: 0, y: 0 }, segments: 4, heights: vec![10.0; 25] }
    }

    #[test]
    fn distance_to_a_motorway_grows_away_from_it() {
        // vertical motorway down the middle of a 4096-extent tile
        let vt = mvt::Tile {
            layers: vec![mvt::Layer { name: "transportation".into(), extent: 4096, features: vec![line("motorway", vec![[2048.0, 0.0], [2048.0, 4096.0]])] }],
        };
        let m = SoundMap::build(&vt, &flat_grid(), 2_400.0);
        let near = m.sample(0.5, 0.5).dist[src::MOTORWAY];
        let mid = m.sample(0.7, 0.5).dist[src::MOTORWAY];
        let far = m.sample(0.95, 0.5).dist[src::MOTORWAY];
        assert!(near < 40.0, "{near}");
        assert!((mid - 480.0).abs() < 60.0, "{mid}");
        assert!(far > mid);
        assert_eq!(m.sample(0.5, 0.5).dist[src::RAIL], FAR_M);
    }

    #[test]
    fn polygons_fill_with_holes() {
        let ring = |a: f32, b: f32| vec![[a, a], [b, a], [b, b], [a, b]];
        let mut hits = vec![false; RES * RES];
        fill_polygon(&[ring(0.0, 4096.0), ring(1024.0, 3072.0)], 4096.0, &mut |i| hits[i] = true);
        assert!(hits[2 * RES + 2]);
        assert!(!hits[32 * RES + 32], "hole stays empty");
    }
}
