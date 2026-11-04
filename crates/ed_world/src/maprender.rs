//! 2D map tiles for the menu map, drawn from the same data the 3D world uses:
//! hillshade from elevation, land cover, water, parks, roads, buildings, and
//! place labels (countries, states, cities, towns, villages).
//!
//! Output is a 256x256 RGBA image plus label anchors; the game draws labels
//! as UI text on top so they stay crisp at any zoom.

use ed_data::elevation::HeightTile;
use ed_data::mvt::{self, GeomType};
use ed_geo::tile::TileId;

pub const SIZE: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlaceClass {
    Continent,
    Country,
    State,
    City,
    Town,
    Village,
}

impl PlaceClass {
    fn from_omt(c: &str) -> Option<Self> {
        Some(match c {
            "continent" => Self::Continent,
            "country" => Self::Country,
            "state" | "province" => Self::State,
            "city" => Self::City,
            "town" => Self::Town,
            "village" | "hamlet" | "suburb" => Self::Village,
            _ => return None,
        })
    }

    /// Lowest map zoom at which this class is labeled.
    pub fn min_zoom(self) -> u8 {
        match self {
            Self::Continent => 0,
            Self::Country => 2,
            Self::State => 4,
            Self::City => 4,
            Self::Town => 8,
            Self::Village => 11,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Label {
    pub name: String,
    pub class: PlaceClass,
    /// Lower = more important (OMT `rank`).
    pub rank: u32,
    /// Position inside the tile, 0..1.
    pub u: f32,
    pub v: f32,
}

#[derive(Debug, Clone)]
pub struct MapTile {
    pub id: TileId,
    /// RGBA8, sRGB, row 0 = north.
    pub rgba: Vec<u8>,
    pub labels: Vec<Label>,
}

type Rgb = [f32; 3];

fn hex(h: u32) -> Rgb {
    [((h >> 16) & 255) as f32 / 255.0, ((h >> 8) & 255) as f32 / 255.0, (h & 255) as f32 / 255.0]
}

const OCEAN: u32 = 0x9cc9e0;
const LAND: u32 = 0xeef0e2;

struct Canvas {
    px: Vec<Rgb>,
    /// Side length in pixels (square).
    n: usize,
}

impl Canvas {
    fn fill_poly(&mut self, rings: &[Vec<[f64; 2]>], c: Rgb, alpha: f32) {
        let n = self.n;
        let mut xs = Vec::new();
        for y in 0..n {
            let sv = (y as f64 + 0.5) / n as f64;
            xs.clear();
            for ring in rings {
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    if (a[1] <= sv) != (b[1] <= sv) {
                        xs.push(a[0] + (sv - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
                    }
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for pair in xs.chunks_exact(2) {
                let x0 = ((pair[0] * n as f64 - 0.5).ceil().max(0.0)) as usize;
                let x1 = ((pair[1] * n as f64 - 0.5).floor().min(n as f64 - 1.0)) as isize;
                for x in x0 as isize..=x1 {
                    let p = &mut self.px[y * n + x as usize];
                    for k in 0..3 {
                        p[k] = p[k] * (1.0 - alpha) + c[k] * alpha;
                    }
                }
            }
        }
    }

    fn line(&mut self, a: [f64; 2], b: [f64; 2], width: f32, c: Rgb) {
        self.line_a(a, b, width, c, 1.0);
    }

    /// Anti-aliased segment in tile uv, `opacity` scales the coverage.
    fn line_a(&mut self, a: [f64; 2], b: [f64; 2], width: f32, c: Rgb, opacity: f32) {
        let stride = self.n;
        let n = stride as f32;
        let (ax, ay, bx, by) = (a[0] as f32 * n, a[1] as f32 * n, b[0] as f32 * n, b[1] as f32 * n);
        let len = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt().max(0.01);
        let r = width * 0.5;
        let (x0, x1) = ((ax.min(bx) - r - 1.0).max(0.0) as i32, (ax.max(bx) + r + 1.0).min(n - 1.0) as i32);
        let (y0, y1) = ((ay.min(by) - r - 1.0).max(0.0) as i32, (ay.max(by) + r + 1.0).min(n - 1.0) as i32);
        if x1 < x0 || y1 < y0 {
            return;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let t = (((px - ax) * (bx - ax) + (py - ay) * (by - ay)) / (len * len)).clamp(0.0, 1.0);
                let d = ((px - (ax + t * (bx - ax))).powi(2) + (py - (ay + t * (by - ay))).powi(2)).sqrt();
                let cov = (r + 0.5 - d).clamp(0.0, 1.0) * opacity;
                if cov > 0.0 {
                    let p = &mut self.px[y as usize * stride + x as usize];
                    for k in 0..3 {
                        p[k] = p[k] * (1.0 - cov) + c[k] * cov;
                    }
                }
            }
        }
    }
}

/// Render one map tile. `heights` (same tile or an ancestor) gives
/// hillshade and the land/sea split where vector water is missing.
pub fn render(id: TileId, vt: Option<&mvt::Tile>, heights: Option<(&HeightTile, (f64, f64, f64))>) -> MapTile {
    render_at(id, vt, (0.0, 0.0, 1.0), heights)
}

/// Render one map tile from a vector tile that covers a larger area.
/// `src` maps `vt`'s tile-local coordinates into this tile: for a feature at
/// `(u, v)` inside `vt`, its position here is `((u - u0) / s, (v - v0) / s)`.
/// This is what overzooming past the vector source's native zoom uses.
pub fn render_at(id: TileId, vt: Option<&mvt::Tile>, src: (f64, f64, f64), heights: Option<(&HeightTile, (f64, f64, f64))>) -> MapTile {
    let (u0, v0, scale) = src;
    let n = SIZE;
    let mut cv = Canvas { px: vec![hex(LAND); n * n], n };
    // Hillshade + hypsometric tint.
    if let Some((h, (u0, v0, s))) = heights {
        let m_per_px = id.width_m() / n as f64;
        let sample = |x: f64, y: f64| h.sample(u0 + s * x / n as f64, v0 + s * y / n as f64) as f64;
        for y in 0..n {
            for x in 0..n {
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let e = sample(fx, fy);
                let dzdx = (sample(fx + 1.0, fy) - sample(fx - 1.0, fy)) / (2.0 * m_per_px);
                let dzdy = (sample(fx, fy + 1.0) - sample(fx, fy - 1.0)) / (2.0 * m_per_px);
                // Light from the north-west, exaggerated relief at low zoom.
                let ex = (1.0 + (14.0 - id.z as f64).max(0.0) * 0.6) as f32;
                let nx = -dzdx as f32 * ex;
                let ny = dzdy as f32 * ex;
                let shade = ((-nx * 0.6 + ny * 0.6 + 1.0) / (nx * nx + ny * ny + 1.0).sqrt()).clamp(0.3, 1.25);
                let base = if e <= 0.0 {
                    // Ocean depth tint.
                    let d = (-e / 6000.0).clamp(0.0, 1.0) as f32;
                    let a = hex(OCEAN);
                    [a[0] * (1.0 - 0.35 * d), a[1] * (1.0 - 0.25 * d), a[2] * (1.0 - 0.1 * d)]
                } else {
                    let t = (e / 4000.0).clamp(0.0, 1.0) as f32;
                    let low = hex(0xdfe8cf);
                    let mid = hex(0xe8dfc6);
                    let high = hex(0xf6f4f0);
                    if t < 0.5 {
                        let k = t * 2.0;
                        [low[0] + (mid[0] - low[0]) * k, low[1] + (mid[1] - low[1]) * k, low[2] + (mid[2] - low[2]) * k]
                    } else {
                        let k = (t - 0.5) * 2.0;
                        [mid[0] + (high[0] - mid[0]) * k, mid[1] + (high[1] - mid[1]) * k, mid[2] + (high[2] - mid[2]) * k]
                    }
                };
                let s = if e <= 0.0 { 1.0 } else { shade };
                cv.px[y * n + x] = [base[0] * s, base[1] * s, base[2] * s];
            }
        }
    }
    let mut labels = Vec::new();
    if let Some(vt) = vt {
        let rings_of = |f: &mvt::Feature, ext: f64| -> Vec<Vec<[f64; 2]>> {
            f.rings
                .iter()
                .map(|r| {
                    r.iter()
                        .map(|p| [(p[0] as f64 / ext - u0) / scale, (p[1] as f64 / ext - v0) / scale])
                        .collect()
                })
                .collect()
        };
        for (layer, pick) in [("landcover", 0u8), ("landuse", 1), ("park", 2), ("water", 3)] {
            let Some(l) = vt.layer(layer) else { continue };
            let ext = l.extent as f64;
            for f in l.features.iter().filter(|f| f.geom_type == GeomType::Polygon) {
                let class = f.str("class").unwrap_or("");
                let (c, a) = match (pick, class) {
                    (0, "wood" | "forest") => (0xc2dcae, 0.8),
                    (0, "grass") => (0xd6e8c0, 0.6),
                    (0, "farmland") => (0xeef0cc, 0.5),
                    (0, "sand") => (0xf2e6c4, 0.8),
                    (0, "ice") => (0xffffff, 0.9),
                    (0, "rock") => (0xdedbd4, 0.6),
                    (0, "wetland") => (0xcfe2d4, 0.6),
                    (1, "residential" | "suburb" | "neighbourhood") => (0xe6e2dc, 0.7),
                    (1, "commercial" | "retail" | "industrial") => (0xe8dfdf, 0.7),
                    (2, _) => (0xbfe0a8, 0.8),
                    (3, _) => (OCEAN, 1.0),
                    _ => continue,
                };
                cv.fill_poly(&rings_of(f, ext), hex(c), a);
            }
        }
        if let Some(l) = vt.layer("building") {
            let ext = l.extent as f64;
            for f in l.features.iter().filter(|f| f.geom_type == GeomType::Polygon) {
                cv.fill_poly(&rings_of(f, ext), hex(0xd4cec6), 1.0);
            }
        }
        if let Some(l) = vt.layer("transportation") {
            let ext = l.extent as f64;
            let zoom_k = (id.z as f32 - 9.0).max(0.4) * 0.35;
            // Casing first, then fill, minor roads before major.
            let mut feats: Vec<_> = l.features.iter().filter(|f| f.geom_type == GeomType::LineString).collect();
            feats.sort_by_key(|f| road_rank(f.str("class").unwrap_or("")));
            for f in feats {
                let (w, fill) = match f.str("class").unwrap_or("") {
                    "motorway" => (3.0, 0xf6c86a),
                    "trunk" => (2.6, 0xf6d98a),
                    "primary" => (2.3, 0xfdf0b0),
                    "secondary" | "tertiary" => (1.8, 0xffffff),
                    "minor" | "service" => (1.1, 0xffffff),
                    "rail" | "transit" => (0.8, 0xb0a8a0),
                    "path" | "track" => (0.6, 0xe8d8c4),
                    _ => continue,
                };
                let w = w * zoom_k;
                for r in &f.rings {
                    for s in r.windows(2) {
                        let map = |p: [f32; 2]| [(p[0] as f64 / ext - u0) / scale, (p[1] as f64 / ext - v0) / scale];
                        let (a, b) = (map(s[0]), map(s[1]));
                        cv.line(a, b, w + 1.2, hex(0xb9b2a6));
                        cv.line(a, b, w, hex(fill));
                    }
                }
            }
        }
        if let Some(l) = vt.layer("boundary") {
            let ext = l.extent as f64;
            for f in l.features.iter().filter(|f| f.geom_type == GeomType::LineString) {
                let level = f.num("admin_level").unwrap_or(10.0);
                if f.num("maritime").unwrap_or(0.0) > 0.0 || level > 4.0 {
                    continue;
                }
                let (w, c) = if level <= 2.0 { (1.4, 0x9a7aa0) } else { (0.8, 0xbba6c0) };
                for r in &f.rings {
                    for s in r.windows(2) {
                        let map = |p: [f32; 2]| [(p[0] as f64 / ext - u0) / scale, (p[1] as f64 / ext - v0) / scale];
                        cv.line(map(s[0]), map(s[1]), w, hex(c));
                    }
                }
            }
        }
        labels.extend(place_labels(vt, (u0, v0, scale)));
    }
    let rgba = cv
        .px
        .iter()
        .flat_map(|c| {
            let g = |x: f32| (x.clamp(0.0, 1.0) * 255.0) as u8;
            [g(c[0]), g(c[1]), g(c[2]), 255]
        })
        .collect();
    MapTile { id, rgba, labels }
}

/// Place labels (countries, states, cities, ...) of `vt` that fall inside
/// the tile `src` maps to (same `(u0, v0, scale)` convention as [`render_at`]).
pub fn place_labels(vt: &mvt::Tile, (u0, v0, scale): (f64, f64, f64)) -> Vec<Label> {
    let mut out = Vec::new();
    let Some(l) = vt.layer("place") else { return out };
    let ext = l.extent as f64;
    for f in &l.features {
        let Some(class) = f.str("class").and_then(PlaceClass::from_omt) else { continue };
        let Some(p) = f.rings.first().and_then(|r| r.first()) else { continue };
        let (u, v) = ((p[0] as f64 / ext - u0) / scale, (p[1] as f64 / ext - v0) / scale);
        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
            continue;
        }
        let name = f.str("name:en").or(f.str("name_en")).or(f.str("name")).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        out.push(Label { name: name.to_string(), class, rank: f.num("rank").unwrap_or(10.0) as u32, u: u as f32, v: v as f32 });
    }
    out
}

/// Draw map linework (country/state borders, then roads by zoom) straight
/// onto a satellite tile, for the 3D map globe. `rgb` is 8-bit sRGB,
/// `size * size * 3`, row 0 = north. `src` maps `vt` into this tile exactly
/// like [`render_at`]. Styled for photo backgrounds: light strokes over a
/// thin dark halo, translucent so the imagery still reads through.
/// Bilinear 2x upsample of an 8-bit RGB square image (for crisper linework
/// on magnified tiles). Returns the new side length and pixels.
pub fn upsample2(size: usize, rgb: &[u8]) -> (usize, Vec<u8>) {
    let m = size * 2;
    let mut out = vec![0u8; m * m * 3];
    for y in 0..m {
        let fy = ((y as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, (size - 1) as f32);
        let (y0, ty) = (fy as usize, fy.fract());
        let y1 = (y0 + 1).min(size - 1);
        for x in 0..m {
            let fx = ((x as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, (size - 1) as f32);
            let (x0, tx) = (fx as usize, fx.fract());
            let x1 = (x0 + 1).min(size - 1);
            for c in 0..3 {
                let p = |xx: usize, yy: usize| rgb[(yy * size + xx) * 3 + c] as f32;
                let v = (p(x0, y0) * (1.0 - tx) + p(x1, y0) * tx) * (1.0 - ty) + (p(x0, y1) * (1.0 - tx) + p(x1, y1) * tx) * ty;
                out[(y * m + x) * 3 + c] = (v + 0.5) as u8;
            }
        }
    }
    (m, out)
}

pub fn overlay_on_imagery(id: TileId, size: usize, rgb: &mut [u8], vt: &mvt::Tile, src: (f64, f64, f64)) {
    assert_eq!(rgb.len(), size * size * 3);
    let (u0, v0, scale) = src;
    // Line widths are authored for 256 px tiles.
    let px_k = size as f32 / SIZE as f32;
    let lin = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    // Blend in linear light so thin AA edges don't darken.
    let mut cv = Canvas { px: rgb.chunks_exact(3).map(|p| [lin(p[0]), lin(p[1]), lin(p[2])]).collect(), n: size };
    let map = |p: [f32; 2], ext: f64| [(p[0] as f64 / ext - u0) / scale, (p[1] as f64 / ext - v0) / scale];
    let lhex = |h: u32| hex(h).map(|c| lin((c * 255.0) as u8));

    if id.z >= 8 {
        if let Some(l) = vt.layer("transportation") {
            let ext = l.extent as f64;
            let z = id.z as f32;
            let mut feats: Vec<_> = l.features.iter().filter(|f| f.geom_type == GeomType::LineString).collect();
            feats.sort_by_key(|f| road_rank(f.str("class").unwrap_or("")));
            for f in feats {
                // (min zoom, width at z12, colour)
                let (min_z, w, c) = match f.str("class").unwrap_or("") {
                    "motorway" => (8, 1.5, 0xffc85a),
                    "trunk" => (8, 1.3, 0xffdc8a),
                    "primary" => (9, 1.1, 0xfff2c0),
                    "secondary" => (10, 0.9, 0xffffff),
                    "tertiary" => (11, 0.7, 0xffffff),
                    "minor" => (12, 0.45, 0xffffff),
                    "rail" => (11, 0.45, 0xd8d0c8),
                    _ => continue,
                };
                if id.z < min_z {
                    continue;
                }
                let w = w * (1.0 + (z - 12.0) * 0.15).max(0.7) * px_k;
                let (halo, fill) = (lhex(0x1a1a1a), lhex(c));
                for r in &f.rings {
                    for s in r.windows(2) {
                        let (a, b) = (map(s[0], ext), map(s[1], ext));
                        cv.line_a(a, b, w + 0.9 * px_k, halo, 0.22);
                        cv.line_a(a, b, w, fill, 0.7);
                    }
                }
            }
        }
    }
    if let Some(l) = vt.layer("boundary") {
        let ext = l.extent as f64;
        for f in l.features.iter().filter(|f| f.geom_type == GeomType::LineString) {
            let level = f.num("admin_level").unwrap_or(10.0);
            if f.num("maritime").unwrap_or(0.0) > 0.0 || level > 4.0 || (level > 2.0 && id.z < 4) {
                continue;
            }
            let (w, op) = if level <= 2.0 { (1.3, 0.8) } else { (0.8, 0.5) };
            for r in &f.rings {
                for s in r.windows(2) {
                    let (a, b) = (map(s[0], ext), map(s[1], ext));
                    cv.line_a(a, b, (w + 1.2) * px_k, lhex(0x000000), op * 0.4);
                    cv.line_a(a, b, w * px_k, lhex(0xf0e6ff), op);
                }
            }
        }
    }
    let enc = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        let s = if x <= 0.0031308 { x * 12.92 } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
        (s * 255.0 + 0.5) as u8
    };
    for (o, c) in rgb.chunks_exact_mut(3).zip(&cv.px) {
        o.copy_from_slice(&[enc(c[0]), enc(c[1]), enc(c[2])]);
    }
}

fn road_rank(class: &str) -> u8 {
    match class {
        "motorway" => 5,
        "trunk" => 4,
        "primary" => 3,
        "secondary" => 2,
        "tertiary" => 1,
        _ => 0,
    }
}

/// Fetch and render a map tile (blocking; run on a worker).
pub fn fetch_and_render(cache: &ed_data::DiskCache, template: Option<&str>, id: TileId) -> MapTile {
    let ez = id.z.min(11);
    let (anc, u0, v0, s) = id.ancestor(ez);
    let h = ed_data::elevation::fetch(cache, anc).ok();
    // Vector tiles stop at z14; deeper zooms overzoom the z14 tile.
    let (vanc, vu0, vv0, vs) = id.ancestor(id.z.min(ed_data::vector::MAX_ZOOM));
    let vt = template.and_then(|t| ed_data::vector::fetch(cache, t, vanc).ok());
    render_at(id, vt.as_ref(), (vu0, vv0, vs), h.as_ref().map(|h| (h, (u0, v0, s))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_offline_with_land_color() {
        let t = render(TileId::new(3, 4, 2), None, None);
        assert_eq!(t.rgba.len(), SIZE * SIZE * 4);
        assert!(t.labels.is_empty());
    }

    #[test]
    fn overzoom_crops_the_parent_tile() {
        let building = ed_data::mvt::Feature {
            id: None,
            geom_type: ed_data::mvt::GeomType::Polygon,
            props: Default::default(),
            // Top-left quadrant of a 4096-extent parent tile.
            rings: vec![vec![[0.0, 0.0], [2048.0, 0.0], [2048.0, 2048.0], [0.0, 2048.0]]],
        };
        let vt = ed_data::mvt::Tile { layers: vec![ed_data::mvt::Layer { name: "building".into(), extent: 4096, features: vec![building] }] };
        let i = ((SIZE / 2) * SIZE + SIZE / 2) * 4;

        let inside = render_at(TileId::new(15, 0, 0), Some(&vt), (0.0, 0.0, 0.5), None);
        assert_eq!(&inside.rgba[i..i + 3], [0xd4, 0xce, 0xc6]);

        let outside = render_at(TileId::new(15, 0, 0), Some(&vt), (0.5, 0.5, 0.5), None);
        assert_ne!(&outside.rgba[i..i + 3], [0xd4, 0xce, 0xc6]);
    }

    #[test]
    fn imagery_overlay_draws_roads_only_when_zoomed_in() {
        let road = ed_data::mvt::Feature {
            id: None,
            geom_type: ed_data::mvt::GeomType::LineString,
            props: [("class".to_string(), ed_data::mvt::Value::String("primary".into()))].into_iter().collect(),
            // Horizontal line through the middle.
            rings: vec![vec![[0.0, 2048.0], [4096.0, 2048.0]]],
        };
        let vt = ed_data::mvt::Tile { layers: vec![ed_data::mvt::Layer { name: "transportation".into(), extent: 4096, features: vec![road] }] };
        let mid = ((SIZE / 2) * SIZE + SIZE / 2) * 3;
        let mut far = vec![20u8; SIZE * SIZE * 3];
        overlay_on_imagery(TileId::new(5, 0, 0), SIZE, &mut far, &vt, (0.0, 0.0, 1.0));
        assert!(far.iter().all(|&c| c == 20), "no roads at z5");
        let mut near = vec![20u8; SIZE * SIZE * 3];
        overlay_on_imagery(TileId::new(12, 0, 0), SIZE, &mut near, &vt, (0.0, 0.0, 1.0));
        assert!(near[mid] > 150, "road drawn light at z12: {}", near[mid]);
        assert!(near[3] < 25, "corner untouched");
    }

    /// `cargo test -p ed_world maprender -- --ignored`: real data has labels.
    #[test]
    #[ignore]
    fn europe_has_country_labels() {
        let cache = ed_data::DiskCache::new(std::env::temp_dir().join("ed-net-test"));
        let tpl = ed_data::vector::tile_template(&cache).ok();
        let t = fetch_and_render(&cache, tpl.as_deref(), TileId::containing(48.0, 8.0, 4));
        let names: Vec<_> = t.labels.iter().map(|l| (&l.name, l.class)).collect();
        eprintln!("{names:?}");
        assert!(t.labels.iter().any(|l| l.class == PlaceClass::Country));
    }
}
