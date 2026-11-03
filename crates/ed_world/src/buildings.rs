//! Buildings from OpenMapTiles footprints.
//!
//! OSM gives footprint + height (+ sometimes colour). We add: a regional
//! style, floors, a window grid (via facade UVs measured in window cells),
//! shopfronts and awnings in commercial areas, and pitched roofs where the
//! footprint is roughly rectangular and the region builds them.

use crate::biome::Biome;
use crate::mesh::{MeshData, lerp_color, rgb, scale_color};
use crate::project::TileProjector;
use crate::style::{self, BuildingLook, Region, RoofKind};
use crate::surface::{Cover, SurfaceMap};
use crate::terrain::TerrainGrid;
use ed_data::mvt::{self, GeomType};
use glam::{Vec2, Vec3};

/// Meters per window bay horizontally.
pub const BAY_WIDTH: f32 = 3.4;

#[derive(Debug, Default)]
pub struct BuildingMeshes {
    /// Opaque walls; UV = window cells (u along wall, v per floor).
    pub facades: MeshData,
    /// Glass curtain walls on towers; same UV scheme.
    pub glass: MeshData,
    /// Ground-floor shopfronts; v in [0,1] over one floor.
    pub storefronts: MeshData,
    /// Roofs, gable ends, awnings. Vertex-colored, untextured.
    pub roofs: MeshData,
    pub count: usize,
}

impl BuildingMeshes {
    /// All geometry merged, for the physics collider.
    pub fn collider(&self) -> MeshData {
        let mut m = MeshData::default();
        for part in [&self.facades, &self.glass, &self.storefronts, &self.roofs] {
            m.append(part);
        }
        m
    }
}

pub struct BuildingContext<'a> {
    pub proj: &'a TileProjector,
    pub grid: &'a TerrainGrid,
    pub surface: &'a SurfaceMap,
    pub biome: Biome,
}

/// Is this tile a dense urban core? (Drives region variant + shopfront odds.)
pub fn tile_is_dense(vt: &mvt::Tile) -> bool {
    let Some(layer) = vt.layer("building") else { return false };
    let n = layer.features.len();
    let tall = layer.features.iter().filter(|f| f.num("render_height").unwrap_or(0.0) > 15.0).count();
    n > 900 || tall > 120
}

pub fn generate(vt: &mvt::Tile, ctx: &BuildingContext) -> BuildingMeshes {
    let center = ctx.proj.tile.center();
    let (gmin, gmax) = ctx.grid.min_max();
    let region = Region::locate(center.lat_deg, center.lon_deg, ((gmin + gmax) * 0.5) as f64, tile_is_dense(vt));
    generate_in_region(vt, ctx, region)
}

/// Same as [`generate`] with the architectural region forced (lookdev, tests).
pub fn generate_in_region(vt: &mvt::Tile, ctx: &BuildingContext, region: Region) -> BuildingMeshes {
    let mut out = BuildingMeshes::default();
    let Some(layer) = vt.layer("building") else { return out };
    let ext = layer.extent as f64;
    let up = ctx.proj.up();

    for f in &layer.features {
        if f.geom_type != GeomType::Polygon {
            continue;
        }
        if matches!(f.props.get("hide_3d"), Some(mvt::Value::Bool(true))) {
            continue;
        }
        for (outer, holes) in mvt::polygons(f) {
            // Skip parts that are mostly outside this tile (MVT buffers overlap
            // neighbours; the neighbour owns them).
            let (cx, cy) = centroid_px(&outer);
            if !(0.0..ext as f32).contains(&cx) || !(0.0..ext as f32).contains(&cy) {
                continue;
            }
            let seed = f.id.unwrap_or(0) ^ ((cx as u64) << 32) ^ (cy as u64);
            let uv = |p: &[f32; 2]| (p[0] as f64 / ext, p[1] as f64 / ext);
            let grounds: Vec<f32> = outer.iter().map(|p| { let (u, v) = uv(p); ctx.grid.sample(u, v) }).collect();
            let g_min = grounds.iter().cloned().fold(f32::MAX, f32::min);
            let g_avg = grounds.iter().sum::<f32>() / grounds.len() as f32;

            let osm_h = f.num("render_height").map(|h| h as f32);
            let min_h = f.num("render_min_height").unwrap_or(0.0) as f32;
            let height = osm_h.filter(|&h| h > 0.5).unwrap_or_else(|| style::default_height(region, seed));
            if height - min_h < 1.0 {
                continue;
            }
            let (cu, cv) = (cx as f64 / ext, cy as f64 / ext);
            let cover = ctx.surface.cover_at(cu, cv);
            let commercial = matches!(cover, Cover::Commercial) || (cover == Cover::Residential && height > 9.0);
            let osm_colour = f.str("colour").and_then(parse_hex_colour);
            let look = style::look_for(region, ctx.biome, seed, height, commercial, osm_colour);

            let base_h = if min_h > 0.0 { g_avg + min_h } else { g_min - 0.5 };
            let top_h = g_avg + height;
            let ring_lo: Vec<Vec3> = outer.iter().map(|p| { let (u, v) = uv(p); ctx.proj.local(u, v, base_h as f64) }).collect();
            let ring_hi: Vec<Vec3> = outer.iter().map(|p| { let (u, v) = uv(p); ctx.proj.local(u, v, top_h as f64) }).collect();
            let holes_hi: Vec<Vec<Vec3>> = holes
                .iter()
                .map(|r| r.iter().map(|p| { let (u, v) = uv(p); ctx.proj.local(u, v, top_h as f64) }).collect())
                .collect();

            emit_walls(&mut out, &ring_lo, &ring_hi, up, &look, min_h > 0.0, g_avg - base_h);
            for (h_lo, h_hi) in holes.iter().map(|r| {
                let lo: Vec<Vec3> = r.iter().map(|p| { let (u, v) = uv(p); ctx.proj.local(u, v, base_h as f64) }).collect();
                let hi: Vec<Vec3> = r.iter().map(|p| { let (u, v) = uv(p); ctx.proj.local(u, v, top_h as f64) }).collect();
                (lo, hi)
            }) {
                emit_walls(&mut out, &h_lo, &h_hi, up, &look, true, 0.0);
            }
            emit_roof(&mut out, &ring_hi, &holes_hi, up, &look, seed);
            out.count += 1;
        }
    }
    out
}

fn centroid_px(r: &[[f32; 2]]) -> (f32, f32) {
    let n = r.len() as f32;
    (r.iter().map(|p| p[0]).sum::<f32>() / n, r.iter().map(|p| p[1]).sum::<f32>() / n)
}

fn parse_hex_colour(s: &str) -> Option<[f32; 4]> {
    let h = s.strip_prefix('#')?;
    (h.len() == 6).then(|| u32::from_str_radix(h, 16).ok().map(rgb)).flatten()
}

/// Signed area in the plane perpendicular to `up` (x/z when up = +Y).
fn plan_area(ring: &[Vec3], up: Vec3) -> f32 {
    let (ax, az) = plan_axes(up);
    let mut a = 0.0;
    for i in 0..ring.len() {
        let (p, q) = (ring[i], ring[(i + 1) % ring.len()]);
        a += p.dot(ax) * q.dot(az) - q.dot(ax) * p.dot(az);
    }
    a * 0.5
}

/// Two horizontal axes forming a right-handed (ax, up, az) basis, like (x, y, z).
fn plan_axes(up: Vec3) -> (Vec3, Vec3) {
    let ax = Vec3::X.reject_from_normalized(up).normalize_or(Vec3::X);
    let az = ax.cross(up);
    (ax, az)
}

fn emit_walls(out: &mut BuildingMeshes, lo: &[Vec3], hi: &[Vec3], up: Vec3, look: &BuildingLook, floating: bool, ground_offset: f32) {
    let ccw = plan_area(lo, up) > 0.0;
    let n = lo.len();
    let mut along = 0.0f32;
    for i in 0..n {
        let j = (i + 1) % n;
        let (a0, b0, a1, b1) = (lo[i], lo[j], hi[i], hi[j]);
        let len = (b0 - a0).length();
        if len < 0.05 {
            continue;
        }
        let wall_h = (a1 - a0).length();
        // Facade UVs: u in bays, v in floors measured from the street.
        let (u0, u1) = (along / BAY_WIDTH, (along + len) / BAY_WIDTH);
        along += len;
        let fh = look.floor_height;
        let v_lo = -ground_offset / fh;
        let v_hi = (wall_h - ground_offset) / fh;
        // Quad order so the normal faces outward.
        let order = |p: [Vec3; 4], uv: [[f32; 2]; 4]| -> ([Vec3; 4], [[f32; 2]; 4]) {
            if ccw { ([p[1], p[0], p[3], p[2]], [uv[1], uv[0], uv[3], uv[2]]) } else { (p, uv) }
        };
        let shop = look.shopfront && !floating && len > 4.0 && wall_h > fh * 1.5;
        if shop {
            // Ground floor shopfront.
            let gf = ground_offset + fh;
            let a_g = a0 + up * gf;
            let b_g = b0 + up * gf;
            let (p, uv) = order([a0, b0, b_g, a_g], [[u0, 0.0], [u1, 0.0], [u1, 1.0], [u0, 1.0]]);
            out.storefronts.quad(p, uv, look.wall);
            let (p, uv) = order([a_g, b_g, b1, a1], [[u0, 1.0], [u1, 1.0], [u1, v_hi], [u0, v_hi]]);
            emit_facade(out, p, uv, look);
            // Awning: a sloped strip sticking out above the shop windows.
            let d = (b0 - a0).normalize();
            let outward = if ccw { up.cross(d) } else { d.cross(up) };
            let h_top = ground_offset + fh * 0.92;
            let inset = 0.6f32.min(len * 0.1);
            let (s0, s1) = (a0 + d * inset + up * h_top, b0 - d * inset + up * h_top);
            let (e0, e1) = (s0 + outward * 1.3 - up * 0.5, s1 + outward * 1.3 - up * 0.5);
            let c = look.awning;
            let (p, uv) = order([s0, s1, e1, e0], [[0.0; 2]; 4]);
            out.roofs.quad(p, uv, c);
            let (p, uv) = order([e0, e1, s1, s0], [[0.0; 2]; 4]);
            out.roofs.quad(p, uv, scale_color(c, 0.7));
        } else {
            let (p, uv) = order([a0, b0, b1, a1], [[u0, v_lo], [u1, v_lo], [u1, v_hi], [u0, v_hi]]);
            emit_facade(out, p, uv, look);
        }
    }
}

fn emit_facade(out: &mut BuildingMeshes, p: [Vec3; 4], uv: [[f32; 2]; 4], look: &BuildingLook) {
    if look.glassy { out.glass.quad(p, uv, look.wall) } else { out.facades.quad(p, uv, look.wall) }
}

fn emit_roof(out: &mut BuildingMeshes, ring: &[Vec3], holes: &[Vec<Vec3>], up: Vec3, look: &BuildingLook, seed: u64) {
    if look.roof_kind != RoofKind::Flat && holes.is_empty() && ring.len() <= 12 {
        if let Some(obb) = Obb::fit(ring, up) {
            if obb.rectangularity > 0.82 && obb.half.y > 2.0 {
                pitched_roof(out, &obb, up, look);
                return;
            }
        }
    }
    flat_roof(out, ring, holes, up, look, seed);
}

fn flat_roof(out: &mut BuildingMeshes, ring: &[Vec3], holes: &[Vec<Vec3>], up: Vec3, look: &BuildingLook, seed: u64) {
    let (ax, az) = plan_axes(up);
    let mut flat = Vec::new();
    let mut hole_idx = Vec::new();
    let mut pts: Vec<Vec3> = Vec::new();
    for p in ring {
        flat.extend_from_slice(&[p.dot(ax) as f64, p.dot(az) as f64]);
        pts.push(*p);
    }
    for h in holes {
        hole_idx.push(pts.len());
        for p in h {
            flat.extend_from_slice(&[p.dot(ax) as f64, p.dot(az) as f64]);
            pts.push(*p);
        }
    }
    let Ok(tris) = earcutr::earcut(&flat, &hole_idx, 2) else { return };
    // Gravel/tar roofs: slight tint variation per building.
    let c = lerp_color(look.roof, rgb(0x9a958c), 0.35 + 0.3 * crate::hash01(seed, 11));
    for t in tris.chunks_exact(3) {
        let (a, b, d) = (pts[t[0]], pts[t[1]], pts[t[2]]);
        if (b - a).cross(d - a).dot(up) >= 0.0 {
            out.roofs.triangle([a, b, d], [[0.0; 2]; 3], c);
        } else {
            out.roofs.triangle([a, d, b], [[0.0; 2]; 3], c);
        }
    }
}

/// Oriented bounding box in the roof plane.
struct Obb {
    center: Vec3,
    /// Long axis, short axis (unit, horizontal).
    axis_l: Vec3,
    axis_s: Vec3,
    /// Half extents along (long, short).
    half: Vec2,
    rectangularity: f32,
}

impl Obb {
    fn fit(ring: &[Vec3], up: Vec3) -> Option<Obb> {
        let area = plan_area(ring, up).abs();
        if area < 1.0 {
            return None;
        }
        let mut best: Option<(f32, Vec3, Vec3, Vec2, Vec2)> = None;
        for i in 0..ring.len() {
            let e = (ring[(i + 1) % ring.len()] - ring[i]).reject_from_normalized(up);
            if e.length() < 0.5 {
                continue;
            }
            let a = e.normalize();
            let b = up.cross(a).normalize();
            let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
            for p in ring {
                let q = Vec2::new(p.dot(a), p.dot(b));
                lo = lo.min(q);
                hi = hi.max(q);
            }
            let ext = hi - lo;
            let box_area = ext.x * ext.y;
            if best.as_ref().is_none_or(|b| box_area < b.0) {
                best = Some((box_area, a, b, lo, hi));
            }
        }
        let (box_area, a, b, lo, hi) = best?;
        let mid = (lo + hi) * 0.5;
        let height = ring.iter().map(|p| p.dot(up)).sum::<f32>() / ring.len() as f32;
        let center = a * mid.x + b * mid.y + up * height;
        let ext = (hi - lo) * 0.5;
        let (axis_l, axis_s, half) = if ext.x >= ext.y { (a, b, ext) } else { (b, a, Vec2::new(ext.y, ext.x)) };
        Some(Obb { center, axis_l, axis_s, half, rectangularity: area / box_area })
    }
}

fn pitched_roof(out: &mut BuildingMeshes, obb: &Obb, up: Vec3, look: &BuildingLook) {
    let overhang = 0.4;
    let (hl, hs) = (obb.half.x + overhang, obb.half.y + overhang);
    let rise = (look.pitch * obb.half.y).min(9.0);
    let (l, s, c) = (obb.axis_l, obb.axis_s, obb.center);
    // Eave drops slightly with the overhang so the slope is continuous.
    let drop = rise * overhang / obb.half.y;
    let eave = |sl: f32, ss: f32| c + l * sl * hl + s * ss * hs - up * drop;
    let roof = look.roof;
    let dark = scale_color(roof, 0.82);
    // Make a quad/triangle face outward relative to the box center.
    let outward_quad = |m: &mut MeshData, p: [Vec3; 4], col: [f32; 4]| {
        let mid = (p[0] + p[1] + p[2] + p[3]) * 0.25;
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        let p = if n.dot(mid - (c + up * rise * 0.3)) >= 0.0 { p } else { [p[1], p[0], p[3], p[2]] };
        m.quad(p, [[0.0; 2]; 4], col);
    };
    let outward_tri = |m: &mut MeshData, p: [Vec3; 3], col: [f32; 4]| {
        let mid = (p[0] + p[1] + p[2]) / 3.0;
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        let p = if n.dot(mid - (c + up * rise * 0.3)) >= 0.0 { p } else { [p[0], p[2], p[1]] };
        m.triangle(p, [[0.0; 2]; 3], col);
    };
    match look.roof_kind {
        RoofKind::Gable => {
            let r0 = c + l * hl + up * rise;
            let r1 = c - l * hl + up * rise;
            outward_quad(&mut out.roofs, [eave(1.0, 1.0), eave(-1.0, 1.0), r1, r0], roof);
            outward_quad(&mut out.roofs, [eave(-1.0, -1.0), eave(1.0, -1.0), r0, r1], dark);
            // Gable ends in the wall color, at the wall line (no overhang).
            let w = look.wall;
            let g = |sl: f32, ss: f32| c + l * sl * obb.half.x + s * ss * obb.half.y;
            outward_tri(&mut out.roofs, [g(1.0, 1.0), g(1.0, -1.0), c + l * obb.half.x + up * rise], w);
            outward_tri(&mut out.roofs, [g(-1.0, -1.0), g(-1.0, 1.0), c - l * obb.half.x + up * rise], w);
        }
        RoofKind::Hip | RoofKind::Flat => {
            let ridge = (hl - hs).max(0.0);
            let r0 = c + l * ridge + up * rise;
            let r1 = c - l * ridge + up * rise;
            outward_quad(&mut out.roofs, [eave(1.0, 1.0), eave(-1.0, 1.0), r1, r0], roof);
            outward_quad(&mut out.roofs, [eave(-1.0, -1.0), eave(1.0, -1.0), r0, r1], dark);
            outward_tri(&mut out.roofs, [eave(1.0, -1.0), eave(1.0, 1.0), r0], scale_color(roof, 0.92));
            outward_tri(&mut out.roofs, [eave(-1.0, 1.0), eave(-1.0, -1.0), r1], scale_color(roof, 0.88));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_geo::{EnuFrame, Geodetic, tile::TileId};

    fn square_tile(height: f64, colour: Option<&str>) -> mvt::Tile {
        let mut props = std::collections::HashMap::new();
        props.insert("render_height".to_string(), mvt::Value::Double(height));
        if let Some(c) = colour {
            props.insert("colour".to_string(), mvt::Value::String(c.into()));
        }
        // ~12 m x 8 m house in the middle of a z14 tile (extent 4096, ~1.7 km).
        let (x0, y0, w, h) = (2000.0, 2000.0, 29.0, 19.0);
        mvt::Tile {
            layers: vec![mvt::Layer {
                name: "building".into(),
                extent: 4096,
                features: vec![mvt::Feature {
                    id: Some(7),
                    geom_type: GeomType::Polygon,
                    props,
                    rings: vec![vec![[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]]],
                }],
            }],
        }
    }

    fn ctx_for(lat: f64, lon: f64, f: impl FnOnce(&BuildingContext) -> BuildingMeshes) -> BuildingMeshes {
        let tile = TileId::containing(lat, lon, 14);
        let proj = TileProjector::new(tile, EnuFrame::new(Geodetic::new(lat, lon, 0.0)));
        let grid = TerrainGrid::flat(tile, 8, 100.0);
        let surface = SurfaceMap::uniform(tile, Biome::Mediterranean);
        f(&BuildingContext { proj: &proj, grid: &grid, surface: &surface, biome: Biome::Mediterranean })
    }

    #[test]
    fn provence_house_gets_pitched_roof_and_outward_walls() {
        let vt = square_tile(7.0, None);
        let m = ctx_for(43.53, 5.45, |c| generate(&vt, c));
        assert_eq!(m.count, 1);
        assert!(!m.facades.is_empty());
        // 4 walls -> 8 triangles; roof is pitched (hip or gable => > 2 tris).
        assert_eq!(m.facades.triangle_count(), 8);
        assert!(m.roofs.triangle_count() >= 6, "roof tris {}", m.roofs.triangle_count());
        // Every wall normal points away from the footprint center.
        let center: Vec3 = m.facades.positions.iter().map(|p| Vec3::from(*p)).sum::<Vec3>() / m.facades.positions.len() as f32;
        for (p, n) in m.facades.positions.iter().zip(&m.facades.normals) {
            let d = (Vec3::from(*p) - center).normalize();
            assert!(Vec3::from(*n).dot(d) > 0.0, "inward wall normal");
        }
    }

    #[test]
    fn manhattan_tower_is_flat_and_tall() {
        let vt = square_tile(180.0, None);
        let m = ctx_for(40.75, -73.98, |c| generate(&vt, c));
        assert_eq!(m.roofs.triangle_count(), 2, "flat roof = 2 tris");
        let all: Vec<_> = m.facades.positions.iter().chain(&m.glass.positions).chain(&m.storefronts.positions).collect();
        let ys: Vec<f32> = all.iter().map(|p| p[1]).collect();
        let span = ys.iter().cloned().fold(f32::MIN, f32::max) - ys.iter().cloned().fold(f32::MAX, f32::min);
        assert!((175.0..185.0).contains(&span), "{span}");
    }

    #[test]
    fn osm_colour_wins() {
        let vt = square_tile(6.0, Some("#ff0000"));
        let m = ctx_for(43.53, 5.45, |c| generate(&vt, c));
        assert!(m.facades.colors[0][0] > 0.9 && m.facades.colors[0][1] < 0.05);
    }
}
