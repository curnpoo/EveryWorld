//! Buildings from OpenMapTiles footprints, modeled in detail.
//!
//! OSM gives the footprint and height. The region's architecture kit
//! (`archkit.rs`) says how such a building is built locally. This module
//! turns that into geometry:
//!
//! 1. Massing: footprint cleaned up, wound consistently, heights snapped to
//!    a floor grid so every window lines up with its floor.
//! 2. Facades: each wall is split into bays sized to its real length.
//!    Walls shared with a neighbor (party walls) stay blank. The wall that
//!    faces the nearest road is the front: doors, stoops, shopfronts and
//!    fire escapes go there.
//! 3. Details: modeled windows, doors, balconies, shutters, AC units,
//!    cornices, string courses, plinths (see `bparts.rs`).
//! 4. Roofs: flat with parapet and rooftop gear, gable/hip with real
//!    overhangs, fascia and tile courses, or Paris mansards with dormers
//!    aligned to the windows below.
//! 5. Lots for detached houses: porch, garage, driveway, walkway, mailbox,
//!    picket fence.

use crate::archkit::{self, Balcony, Cornice, GroundFloor, RoofForm, RoofMat, Variant, WallMat};
use crate::biome::Biome;
use crate::bparts::{self as bp, Mat, Out};
use crate::mesh::{MeshData, lerp_color, rgb, scale_color};
use crate::meshkit::{self as mk, Arch, Basis, Opening, profiles};
use crate::project::TileProjector;
use crate::roads::RoadClass;
use crate::style::Region;
use crate::surface::{Cover, SurfaceMap};
use crate::terrain::TerrainGrid;
use ed_data::mvt::{self, GeomType};
use glam::{Vec2, Vec3};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct BuildingMeshes {
    /// Render geometry bucketed by material.
    pub out: Out,
    /// Simple shells (walls + roof caps) for physics.
    pub collider: MeshData,
    pub count: usize,
}

impl BuildingMeshes {
    pub fn collider(&self) -> MeshData {
        self.collider.clone()
    }
    pub fn triangle_count(&self) -> usize {
        self.out.triangle_count()
    }
}

pub struct BuildingContext<'a> {
    pub proj: &'a TileProjector,
    pub grid: &'a TerrainGrid,
    pub surface: &'a SurfaceMap,
    pub biome: Biome,
    /// Road centerlines in the same local space (for "which side is the street").
    pub roads: &'a [(RoadClass, Vec<Vec3>)],
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
/// Builds everything at full detail.
pub fn generate_in_region(vt: &mvt::Tile, ctx: &BuildingContext, region: Region) -> BuildingMeshes {
    let specs = prepare(vt, ctx, region);
    let dctx = DetailCtx { proj: *ctx.proj, grid: ctx.grid.clone(), roads: ctx.roads.to_vec() };
    build_detail(specs.iter(), &dctx)
}

/// Grid of detail chunks per tile side (8 x 8 chunks of ~250-300 m).
pub const CHUNKS: u8 = 8;

/// Everything decided about one building, before any geometry is made.
/// Cheap to compute for a whole tile; geometry is built per chunk on demand.
#[derive(Debug, Clone)]
pub struct BuildingSpec {
    f: Footprint,
    ring: Vec<Vec3>,
    walls: Vec<Wall>,
    var: &'static Variant,
    region: Region,
    floors: u32,
    upper: bool,
    gy: f32,
    ground_h: f32,
    wall_top: f32,
    area: f32,
    commercial: bool,
    balcony: bool,
    fire_escape: bool,
    wall_c: [f32; 4],
    trim_c: [f32; 4],
    door_c: [f32; 4],
    roof_c: [f32; 4],
    shutter_c: Option<[f32; 4]>,
    pub chunk: (u8, u8),
    pub center: Vec3,
}

/// What the detail builder needs (owned, so chunks can build on any thread).
#[derive(Debug, Clone)]
pub struct DetailCtx {
    pub proj: TileProjector,
    pub grid: TerrainGrid,
    pub roads: Vec<(RoadClass, Vec<Vec3>)>,
}

/// Decide every building in a tile (variant, floors, front, colors...).
pub fn prepare(vt: &mvt::Tile, ctx: &BuildingContext, region: Region) -> Vec<BuildingSpec> {
    let mut specs = Vec::new();
    let Some(layer) = vt.layer("building") else { return specs };
    let ext = layer.extent as f64;
    let up = ctx.proj.up();
    let index = RoadIndex::new(ctx.roads, up);

    // Party walls: edges shared (reversed) by two footprints.
    let mut edge_count: HashMap<((i32, i32), (i32, i32)), u8> = HashMap::new();
    let key = |p: [f32; 2]| ((p[0] * 2.0).round() as i32, (p[1] * 2.0).round() as i32);
    for f in &layer.features {
        for ring in &f.rings {
            for i in 0..ring.len() {
                let (a, b) = (key(ring[i]), key(ring[(i + 1) % ring.len()]));
                let k = if a < b { (a, b) } else { (b, a) };
                *edge_count.entry(k).or_default() += 1;
            }
        }
    }

    for f in &layer.features {
        if f.geom_type != GeomType::Polygon || matches!(f.props.get("hide_3d"), Some(mvt::Value::Bool(true))) {
            continue;
        }
        for (outer, holes) in mvt::polygons(f) {
            let n = outer.len() as f32;
            let (cx, cy) = (outer.iter().map(|p| p[0]).sum::<f32>() / n, outer.iter().map(|p| p[1]).sum::<f32>() / n);
            if !(0.0..ext as f32).contains(&cx) || !(0.0..ext as f32).contains(&cy) {
                continue; // the neighbouring tile owns it
            }
            let seed = f.id.unwrap_or(0) ^ ((cx as u64) << 32) ^ (cy as u64);
            let party: Vec<bool> = (0..outer.len())
                .map(|i| {
                    let (a, b) = (key(outer[i]), key(outer[(i + 1) % outer.len()]));
                    let k = if a < b { (a, b) } else { (b, a) };
                    edge_count.get(&k).copied().unwrap_or(0) > 1
                })
                .collect();
            let fp = Footprint {
                ring: outer.iter().map(|p| (p[0] as f64 / ext, p[1] as f64 / ext)).collect(),
                party,
                holes: !holes.is_empty(),
                height: f.num("render_height").map(|h| h as f32).filter(|h| *h > 0.5),
                min_height: f.num("render_min_height").unwrap_or(0.0) as f32,
                colour: f.str("colour").and_then(parse_hex_colour),
                seed,
                center: (cx as f64 / ext, cy as f64 / ext),
            };
            let chunk = (((cx as f64 / ext) * CHUNKS as f64) as u8).min(CHUNKS - 1);
            let chunk_y = (((cy as f64 / ext) * CHUNKS as f64) as u8).min(CHUNKS - 1);
            if let Some(mut spec) = prepare_one(fp, ctx, &index, region, up) {
                spec.chunk = (chunk, chunk_y);
                specs.push(spec);
            }
        }
    }
    specs
}

/// Full-detail geometry for a set of buildings.
pub fn build_detail<'a>(specs: impl IntoIterator<Item = &'a BuildingSpec>, dctx: &DetailCtx) -> BuildingMeshes {
    let mut out = BuildingMeshes::default();
    let up = dctx.proj.up();
    for s in specs {
        detail_one(&mut out, s, dctx, up);
        out.count += 1;
    }
    out
}

/// Distant version: massing with windows drawn by texture, aligned to the
/// same bay/floor layout as the detailed model, simple roofs.
pub fn build_lod<'a>(specs: impl IntoIterator<Item = &'a BuildingSpec>, up: Vec3) -> BuildingMeshes {
    let mut out = BuildingMeshes::default();
    for s in specs {
        lod_one(&mut out, s, up);
        out.count += 1;
    }
    out
}

#[derive(Debug, Clone)]
struct Footprint {
    ring: Vec<(f64, f64)>,
    party: Vec<bool>,
    holes: bool,
    height: Option<f32>,
    min_height: f32,
    colour: Option<[f32; 4]>,
    seed: u64,
    center: (f64, f64),
}

fn parse_hex_colour(s: &str) -> Option<[f32; 4]> {
    let h = s.strip_prefix('#')?;
    (h.len() == 6).then(|| u32::from_str_radix(h, 16).ok().map(rgb)).flatten()
}

fn pick<T: Copy>(xs: &[T], seed: u64, salt: u64) -> T {
    xs[((crate::hash01(seed, salt) * xs.len() as f32) as usize).min(xs.len() - 1)]
}

/// Plan-view helpers.
fn plan_dist_to_segment(p: Vec3, a: Vec3, b: Vec3, up: Vec3) -> (f32, Vec3) {
    let (p, a, b) = (p.reject_from_normalized(up), a.reject_from_normalized(up), b.reject_from_normalized(up));
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    let q = a + ab * t;
    (p.distance(q), q)
}

/// Road segments bucketed on a 40 m grid for fast "nearest street" queries.
pub struct RoadIndex {
    cells: HashMap<(i32, i32), Vec<(Vec3, Vec3, RoadClass)>>,
    ax: Vec3,
    az: Vec3,
}

const ROAD_CELL: f32 = 40.0;

impl RoadIndex {
    pub fn new(roads: &[(RoadClass, Vec<Vec3>)], up: Vec3) -> Self {
        let ax = Vec3::X.reject_from_normalized(up).normalize_or(Vec3::X);
        let az = ax.cross(up);
        let mut cells: HashMap<(i32, i32), Vec<(Vec3, Vec3, RoadClass)>> = HashMap::new();
        for (class, line) in roads {
            if !class.is_car_road() && *class != RoadClass::Path {
                continue;
            }
            for w in line.windows(2) {
                let (a, b) = (w[0], w[1]);
                let (x0, x1) = (a.dot(ax).min(b.dot(ax)), a.dot(ax).max(b.dot(ax)));
                let (z0, z1) = (a.dot(az).min(b.dot(az)), a.dot(az).max(b.dot(az)));
                for cx in (x0 / ROAD_CELL).floor() as i32..=(x1 / ROAD_CELL).floor() as i32 {
                    for cz in (z0 / ROAD_CELL).floor() as i32..=(z1 / ROAD_CELL).floor() as i32 {
                        cells.entry((cx, cz)).or_default().push((a, b, *class));
                    }
                }
            }
        }
        Self { cells, ax, az }
    }

    pub fn nearest(&self, p: Vec3, up: Vec3) -> Option<(f32, Vec3, RoadClass)> {
        let (cx, cz) = ((p.dot(self.ax) / ROAD_CELL).floor() as i32, (p.dot(self.az) / ROAD_CELL).floor() as i32);
        let mut best: Option<(f32, Vec3, RoadClass)> = None;
        for r in 1..=2 {
            for dx in -r..=r {
                for dz in -r..=r {
                    if let Some(segs) = self.cells.get(&(cx + dx, cz + dz)) {
                        for (a, b, class) in segs {
                            let (d, q) = plan_dist_to_segment(p, *a, *b, up);
                            if best.is_none_or(|bb| d < bb.0) {
                                best = Some((d, q, *class));
                            }
                        }
                    }
                }
            }
            if best.is_some_and(|b| b.0 < ROAD_CELL * r as f32) {
                break;
            }
        }
        best
    }
}

fn nearest_road(p: Vec3, roads: &[(RoadClass, Vec<Vec3>)], up: Vec3) -> Option<(f32, Vec3, RoadClass)> {
    let mut best: Option<(f32, Vec3, RoadClass)> = None;
    for (class, line) in roads {
        if !class.is_car_road() && *class != RoadClass::Path {
            continue;
        }
        for w in line.windows(2) {
            let (d, q) = plan_dist_to_segment(p, w[0], w[1], up);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, q, *class));
            }
        }
    }
    best
}

/// Remove tiny edges and near-collinear vertices so walls are clean.
fn simplify(ring: &[Vec3], party: &[bool], up: Vec3) -> (Vec<Vec3>, Vec<bool>) {
    let mut pts: Vec<(Vec3, bool)> = ring.iter().copied().zip(party.iter().copied()).collect();
    let mut changed = true;
    while changed && pts.len() > 3 {
        changed = false;
        for i in 0..pts.len() {
            let n = pts.len();
            let prev = pts[(i + n - 1) % n].0;
            let cur = pts[i].0;
            let next = pts[(i + 1) % n].0;
            let a = (cur - prev).reject_from_normalized(up);
            let b = (next - cur).reject_from_normalized(up);
            let tiny = a.length() < 0.35;
            let straight = a.length() > 1e-4 && b.length() > 1e-4 && a.normalize().dot(b.normalize()) > 0.996;
            if tiny || straight {
                // Merge: the edge (prev -> cur) is absorbed into (cur -> next) or vice versa.
                let party_prev = pts[(i + n - 1) % n].1;
                pts[(i + n - 1) % n].1 = party_prev && pts[i].1;
                pts.remove(i);
                changed = true;
                break;
            }
        }
    }
    (pts.iter().map(|p| p.0).collect(), pts.iter().map(|p| p.1).collect())
}

/// One wall segment and how it is used.
#[derive(Debug, Clone)]
struct Wall {
    a: Vec3,
    b: Vec3,
    len: f32,
    outward: Vec3,
    party: bool,
    front: bool,
    street: bool,
}

fn prepare_one(f: Footprint, ctx: &BuildingContext, index: &RoadIndex, region: Region, up: Vec3) -> Option<BuildingSpec> {
    let grounds: Vec<f32> = f.ring.iter().map(|&(u, v)| ctx.grid.sample(u, v)).collect();
    let g_min = grounds.iter().cloned().fold(f32::MAX, f32::min);
    let g_avg = grounds.iter().sum::<f32>() / grounds.len() as f32;
    let neighborhood = {
        let c = ctx.proj.tile.uv_to_geodetic(f.center.0, f.center.1);
        ((c.lat_deg * 450.0).floor() as i64 as u64).wrapping_mul(0x9E37_79B9) ^ ((c.lon_deg * 450.0).floor() as i64 as u64)
    };
    let var = archkit::pick(region, neighborhood, f.seed);
    let floors = match f.height {
        Some(h) => (((h - f.min_height - var.ground_h) / var.floor_h).round() as i32 + 1).clamp(1, 90) as u32,
        None => {
            let (lo, hi) = var.floors;
            lo as u32 + (crate::hash01(f.seed, 9) * (hi - lo + 1) as f32).floor() as u32
        }
    };
    let upper = f.min_height > 0.5;
    let base_h = if upper { g_avg + f.min_height } else { g_min - 0.4 };
    // Facade y is measured from base_h. `gy` = street level in facade space.
    let gy = if upper { 0.0 } else { g_avg - base_h };
    let stoop = var.ground == GroundFloor::Stoop && !upper;
    let ground_h = if upper { var.floor_h } else { var.ground_h + if stoop { 1.2 } else { 0.0 } };
    let wall_top = gy + ground_h + (floors.saturating_sub(1)) as f32 * var.floor_h;

    let mut ring: Vec<Vec3> = f.ring.iter().map(|&(u, v)| ctx.proj.local(u, v, base_h as f64)).collect();
    let mut party = f.party.clone();
    if mk::signed_area(&ring, up) < 0.0 {
        ring.reverse();
        // Edge i (ring[i] -> ring[i+1]) after reversal was edge (n-2-i) before.
        let n = party.len();
        party = (0..n).map(|i| f.party[(2 * n - 2 - i) % n]).collect();
    }
    let (ring, party) = simplify(&ring, &party, up);
    if ring.len() < 3 {
        return None;
    }
    let area = mk::signed_area(&ring, up);
    if area < 6.0 {
        return None;
    }
    let centroid = ring.iter().copied().sum::<Vec3>() / ring.len() as f32;
    let road = index.nearest(centroid, up);

    // Colors for this building.
    let wall_c = f.colour.unwrap_or_else(|| rgb(pick(var.walls, f.seed, 1)));
    let trim_c = rgb(pick(var.trim, f.seed, 2));
    let door_c = rgb(pick(var.doors, f.seed, 3));
    let roof_c = rgb(pick(var.roofs, f.seed, 4));
    let shutter_c = (!var.window.shutter_colors.is_empty()).then(|| rgb(pick(var.window.shutter_colors, f.seed, 5)));
    // Subtle per-building weathering so rows of houses aren't identical.
    let wall_c = scale_color(wall_c, 0.93 + 0.1 * crate::hash01(f.seed, 6));

    // Walls: outward = up x dir for a CCW (positive area) ring.
    let n = ring.len();
    let mut walls: Vec<Wall> = (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            let d = (b - a).reject_from_normalized(up);
            let len = d.length();
            Wall { a, b, len, outward: up.cross(d / len.max(1e-4)).normalize_or(Vec3::X), party: party[i], front: false, street: false }
        })
        .collect();
    if let Some((_, q, _)) = road {
        let to_road = (q - centroid).reject_from_normalized(up).normalize_or(Vec3::X);
        let mut best = (f32::MIN, usize::MAX);
        for (i, w) in walls.iter_mut().enumerate() {
            let mid = (w.a + w.b) * 0.5;
            let (dist, _) = index.nearest(mid, up).map(|r| (r.0, r.1)).unwrap_or((999.0, mid));
            w.street = !w.party && w.outward.dot(to_road) > 0.5 && dist < 30.0;
            let score = w.outward.dot(to_road) * w.len.min(12.0) - dist * 0.05;
            if !w.party && w.len > 2.2 && score > best.0 {
                best = (score, i);
            }
        }
        if best.1 != usize::MAX {
            walls[best.1].front = true;
            walls[best.1].street = true;
        }
    } else if let Some(i) = (0..n).filter(|&i| !walls[i].party).max_by(|&a, &b| walls[a].len.total_cmp(&walls[b].len)) {
        walls[i].front = true;
    }

    let commercial = matches!(ctx.surface.cover_at(f.center.0, f.center.1), Cover::Commercial | Cover::Residential | Cover::Industrial)
        && var.ground == GroundFloor::Shops;
    let building_balcony = crate::hash01(f.seed, 21) < var.balcony_odds;
    let has_fire_escape = crate::hash01(f.seed, 22) < var.props.fire_escape && floors >= 3;
    Some(BuildingSpec {
        f,
        ring,
        walls,
        var,
        region,
        floors,
        upper,
        gy,
        ground_h,
        wall_top,
        area,
        commercial,
        balcony: building_balcony,
        fire_escape: has_fire_escape,
        wall_c,
        trim_c,
        door_c,
        roof_c,
        shutter_c,
        chunk: (0, 0),
        center: centroid,
    })
}

fn detail_one(o: &mut BuildingMeshes, s: &BuildingSpec, dctx: &DetailCtx, up: Vec3) {
    let (f, ring, walls, var, region) = (&s.f, &s.ring, &s.walls, s.var, s.region);
    let (floors, upper, gy, ground_h, wall_top, area) = (s.floors, s.upper, s.gy, s.ground_h, s.wall_top, s.area);
    let (commercial, building_balcony, has_fire_escape) = (s.commercial, s.balcony, s.fire_escape);
    let (wall_c, trim_c, door_c, roof_c, shutter_c) = (s.wall_c, s.trim_c, s.door_c, s.roof_c, s.shutter_c);

    // ------------------------------------------------------------ facades
    for w in walls {
        if w.len < 0.8 {
            continue;
        }
        // Basis: origin at b, x toward a, so z = x X up points outward.
        let b = Basis::new(w.b, w.a - w.b, up);
        facade(o, &b, w, var, f, floors, gy, ground_h, wall_top, upper, commercial, building_balcony, has_fire_escape, wall_c, trim_c, door_c, shutter_c);
    }

    // Plinth, string courses, cornice: swept around the whole footprint.
    let outward_sweep = |o: &mut BuildingMeshes, prof: &[Vec2], y: f32, c: [f32; 4]| {
        // Sweep's profile x points to the path's right (= inward for CCW); flip it.
        let flipped: Vec<Vec2> = prof.iter().rev().map(|p| Vec2::new(-p.x, p.y)).collect();
        let path: Vec<Vec3> = ring.iter().map(|p| *p + up * y).collect();
        mk::sweep(o.out.m(Mat::Trim), &flipped, &path, true, up, c);
    };
    if var.plinth && !upper {
        outward_sweep(o, &profiles::plinth(0.08, 0.45), 0.0, lerp_color(wall_c, rgb(0x8a8680), 0.6));
    }
    if var.string_courses {
        for k in 1..floors {
            let y = gy + ground_h + (k - 1) as f32 * var.floor_h;
            outward_sweep(o, &profiles::band(0.07, 0.16), y - 0.08, trim_c);
        }
    }

    // ------------------------------------------------------------ roof
    let form = if f.holes || ring.len() > 24 { RoofForm::Flat } else { pick(var.roof_forms, f.seed, 7) };
    let form = if floors > 8 && form != RoofForm::Mansard { RoofForm::Flat } else { form };
    match form {
        RoofForm::Flat => {
            match var.cornice {
                Cornice::None => {}
                Cornice::Simple => outward_sweep(o, &profiles::band(0.18, 0.3), wall_top - 0.3, trim_c),
                Cornice::Classical => outward_sweep(o, &profiles::cornice(0.45, 0.6), wall_top - 0.55, trim_c),
                Cornice::Bracketed => {
                    outward_sweep(o, &profiles::cornice(0.6, 0.55), wall_top - 0.5, trim_c);
                    brackets(o, walls, up, wall_top - 0.5, trim_c);
                }
            }
            flat_roof(o, ring, walls, up, wall_top, var, f, wall_c, region);
        }
        RoofForm::Mansard => {
            outward_sweep(o, &profiles::cornice(0.45, 0.55), wall_top - 0.5, trim_c);
            mansard(o, ring, walls, up, wall_top, var, f, roof_c, trim_c, wall_c);
        }
        RoofForm::Gable | RoofForm::Hip | RoofForm::LowHip => {
            if var.cornice != Cornice::None {
                outward_sweep(o, &profiles::band(0.12, 0.22), wall_top - 0.22, trim_c);
            }
            pitched(o, ring, walls, up, wall_top, form, var, f, roof_c, trim_c, wall_c);
        }
    }

    // ------------------------------------------------------------ lot
    if var.house && !upper && area < 450.0 {
        lot(o, walls, up, gy, var, f, dctx, trim_c, roof_c, door_c);
    }

}

/// Distant building: textured massing (windows drawn on, aligned to the
/// same bays and floors as the detailed model) and plain roof planes.
fn lod_one(o: &mut BuildingMeshes, s: &BuildingSpec, up: Vec3) {
    let var = s.var;
    let base_c = match var.base {
        Some((c, _)) if !s.upper => rgb(c),
        _ => s.wall_c,
    };
    let split = if s.upper { 0.0 } else { (s.gy + s.ground_h).min(s.wall_top) };
    for w in &s.walls {
        if w.len < 0.5 {
            continue;
        }
        let b = Basis::new(w.b, w.a - w.b, up);
        let m = o.out.m(Mat::LodWall);
        // u in bays (windows at bay centers), v in floors. Piers and party
        // walls sample the plain-wall corner of the texture (u = 0.04).
        let quad = |m: &mut MeshData, x0: f32, x1: f32, y0: f32, y1: f32, u0: f32, u1: f32, v0: f32, v1: f32, c: [f32; 4]| {
            m.quad([b.p(x0, y0, 0.0), b.p(x1, y0, 0.0), b.p(x1, y1, 0.0), b.p(x0, y1, 0.0)], [[u0, v0], [u1, v0], [u1, v1], [u0, v1]], c);
        };
        if w.party {
            quad(m, 0.0, w.len, 0.0, s.wall_top, 0.04, 0.04, 0.5, 0.5, s.wall_c);
            continue;
        }
        let pier_min = 0.35;
        let nb = (((w.len - 2.0 * pier_min) / var.bay_w).floor() as i32).max(if w.len > 1.8 { 1 } else { 0 }) as usize;
        let bay = if nb > 0 { var.bay_w.min((w.len - 2.0 * pier_min) / nb as f32).max(1.2) } else { 0.0 };
        let pier = (w.len - nb as f32 * bay) * 0.5;
        let floors_up = (s.wall_top - split) / var.floor_h;
        for (y0, y1, v0, v1, c) in [(0.0, split, 0.0, 1.0, base_c), (split, s.wall_top, 0.0, floors_up, s.wall_c)] {
            if y1 - y0 < 0.05 {
                continue;
            }
            // Ground band shows windows only above street level.
            let (v0, v1) = if y0 == 0.0 { (-(s.gy) / s.ground_h.max(0.1), 1.0) } else { (v0, v1) };
            quad(m, 0.0, pier, y0, y1, 0.04, 0.04, v0, v1, c);
            quad(m, pier, w.len - pier, y0, y1, 0.0, nb as f32, v0, v1, c);
            quad(m, w.len - pier, w.len, y0, y1, 0.04, 0.04, v0, v1, c);
        }
    }
    // Physics shell: walls + a cap at the eaves.
    for w in &s.walls {
        let p = [w.a, w.b, w.b + up * s.wall_top, w.a + up * s.wall_top];
        o.collider.quad(p, [[0.0; 2]; 4], [1.0; 4]);
    }
    let cap: Vec<Vec3> = s.ring.iter().map(|p| *p + up * s.wall_top).collect();
    mk::polygon(&mut o.collider, &cap, &[], up, 1.0, [1.0; 4]);

    // Roof: flat cap, or a hip made by insetting the eave ring.
    let eave: Vec<Vec3> = s.ring.iter().map(|p| *p + up * s.wall_top).collect();
    let flat = s.f.holes || s.ring.len() > 24 || s.floors > 8 || var.roof_forms.iter().all(|f| *f == RoofForm::Flat);
    if flat {
        let parapet = if matches!(var.cornice, Cornice::None) { 0.55 } else { 0.9 };
        for w in &s.walls {
            let p = [w.a + up * s.wall_top, w.b + up * s.wall_top, w.b + up * (s.wall_top + parapet), w.a + up * (s.wall_top + parapet)];
            o.out.m(Mat::LodWall).quad(p, [[0.04, 0.5]; 4], s.wall_c);
        }
        let deck: Vec<Vec3> = eave.iter().map(|p| *p + up * (parapet * 0.6)).collect();
        mk::polygon(o.out.m(Mat::Roof(RoofMat::Gravel)), &deck, &[], up, 2.0, s.roof_c);
    } else {
        let span = (2.0 * mk::signed_area(&eave, up) / perimeter(&eave)).max(1.0);
        let mansard = var.roof_forms.contains(&RoofForm::Mansard);
        let (d, rise) = if mansard { (1.0, 2.9) } else { (span * 0.95, (var.pitch * span * 0.95).min(6.0)) };
        let inner: Vec<Vec3> = mk::inset(&eave, up, d).iter().map(|p| *p + up * rise).collect();
        let mat = if mansard { RoofMat::Zinc } else { var.roof_mat };
        let m = o.out.m(Mat::Roof(mat));
        for i in 0..eave.len() {
            let j = (i + 1) % eave.len();
            let q = [eave[i], eave[j], inner[j], inner[i]];
            let len = eave[i].distance(eave[j]);
            m.quad(q, [[0.0, 0.0], [len, 0.0], [len, 1.0], [0.0, 1.0]], s.roof_c);
        }
        mk::polygon(m, &inner, &[], up, 1.0, s.roof_c);
    }
}

#[allow(clippy::too_many_arguments)]
fn facade(
    o: &mut BuildingMeshes,
    b: &Basis,
    w: &Wall,
    var: &Variant,
    f: &Footprint,
    floors: u32,
    gy: f32,
    ground_h: f32,
    wall_top: f32,
    upper: bool,
    commercial: bool,
    building_balcony: bool,
    fire_escape: bool,
    wall_c: [f32; 4],
    trim_c: [f32; 4],
    door_c: [f32; 4],
    shutter_c: Option<[f32; 4]>,
) {
    let len = w.len;
    let wseed = f.seed ^ ((w.a.x * 13.0) as i64 as u64) ^ ((w.a.z * 7.0) as i64 as u64).rotate_left(17);
    let reveal_c = scale_color(wall_c, 0.82);
    let base_split = gy + ground_h.min(wall_top);
    let (base_c, base_mat) = match var.base {
        Some((c, m)) if !upper => (rgb(c), m),
        _ => (wall_c, var.wall_mat),
    };
    let uv = match var.wall_mat {
        WallMat::Brick => 1.0,
        WallMat::Siding | WallMat::Timber => 1.2,
        _ => 2.0,
    };
    // Party walls: plain, no openings.
    if w.party {
        let mut rev = MeshData::default();
        mk::wall_with_openings(o.out.m(Mat::Wall(var.wall_mat)), b, len, 0.0, wall_top, &[], 0.2, uv, wall_c, &mut rev, reveal_c);
        return;
    }
    // Bay layout.
    let pier_min = 0.35;
    let nb = (((len - 2.0 * pier_min) / var.bay_w).floor() as i32).max(if len > 1.8 { 1 } else { 0 }) as usize;
    let bay = if nb > 0 { var.bay_w.min((len - 2.0 * pier_min) / nb as f32).max(1.2) } else { 0.0 };
    let pier = (len - nb as f32 * bay) * 0.5;
    let bay_x = |i: usize| pier + (i as f32 + 0.5) * bay;

    let mut ground_ops: Vec<Opening> = Vec::new();
    let mut upper_ops: Vec<Opening> = Vec::new();
    let wd = &var.window;
    let win_w = wd.w.min(bay * 0.72);
    // Ground floor contents.
    let door_bay = if w.front && nb > 0 { Some(nb / 2) } else { None };
    let shop_here = commercial && w.street && !upper;
    let garage_bay = if var.house && w.front && nb >= 3 && len > 8.5 { Some(nb - 1) } else { None };
    enum G {
        Window,
        Door,
        Shop,
        Garage,
        Arcade,
    }
    let mut ground_kinds: Vec<(usize, G, Opening)> = Vec::new();
    if !upper && nb > 0 {
        for i in 0..nb {
            let cx = bay_x(i);
            let kind = if Some(i) == garage_bay {
                G::Garage
            } else if Some(i) == door_bay && !shop_here {
                G::Door
            } else if shop_here && var.ground == GroundFloor::Arcade {
                G::Arcade
            } else if shop_here {
                if Some(i) == door_bay { G::Door } else { G::Shop }
            } else {
                G::Window
            };
            let op = match kind {
                G::Door => {
                    let dw = if var.ground == GroundFloor::Stoop { 1.15 } else { 1.05 };
                    let dy = if var.ground == GroundFloor::Stoop { gy + 1.2 } else { gy + 0.15 };
                    let arch = if matches!(wd.arch, Arch::Pointed | Arch::Round) { wd.arch } else if var.ground == GroundFloor::Stoop { Arch::Segmental(0.15) } else { Arch::Flat };
                    Opening { x0: cx - dw * 0.5, y0: dy, w: dw, h: (ground_h - 0.5).min(2.6).max(2.1), arch }
                }
                G::Shop => Opening { x0: cx - bay * 0.44, y0: gy + 0.05, w: bay * 0.88, h: (ground_h - 0.9).clamp(2.2, 3.4), arch: Arch::Flat },
                G::Arcade => Opening { x0: cx - bay * 0.42, y0: gy + 0.05, w: bay * 0.84, h: (ground_h - 0.4).clamp(2.4, 3.6), arch: Arch::Round },
                G::Garage => {
                    let gw = (bay * 0.9).min(2.8);
                    Opening { x0: cx - gw * 0.5, y0: gy + 0.05, w: gw, h: 2.2, arch: Arch::Flat }
                }
                G::Window => {
                    let sill = if var.ground == GroundFloor::Stoop { gy + 1.4 } else { gy + wd.sill_h.max(0.75) };
                    Opening { x0: cx - win_w * 0.5, y0: sill, w: win_w, h: wd.h.min(ground_h - (sill - gy) - 0.35).max(0.6), arch: wd.arch }
                }
            };
            ground_ops.push(op);
            ground_kinds.push((i, kind, op));
        }
    }
    // Upper floors.
    let first_upper = if upper { 0 } else { 1 };
    let floor_y = |k: u32| if upper { gy + k as f32 * var.floor_h } else { gy + ground_h + (k - 1) as f32 * var.floor_h };
    let mut upper_list: Vec<(u32, usize, Opening)> = Vec::new();
    for k in first_upper..floors {
        let fy = floor_y(k);
        let h = wd.h.min(var.floor_h - wd.sill_h - 0.3).max(0.6);
        for i in 0..nb {
            // Machiya-style or tiny walls: skip some windows for rhythm.
            let op = Opening { x0: bay_x(i) - win_w * 0.5, y0: fy + wd.sill_h, w: win_w, h, arch: wd.arch };
            upper_ops.push(op);
            upper_list.push((k, i, op));
        }
    }

    // Wall surfaces: base band (ground floor) and upper band, with holes.
    let mut reveals = MeshData::default();
    let split = if upper { 0.0 } else { base_split.min(wall_top) };
    if split > 0.05 {
        mk::wall_with_openings(o.out.m(Mat::Wall(base_mat)), b, len, 0.0, split, &ground_ops, 0.24, uv, base_c, &mut reveals, scale_color(base_c, 0.82));
    }
    if wall_top - split > 0.05 {
        mk::wall_with_openings(o.out.m(Mat::Wall(var.wall_mat)), b, len, split, wall_top, &upper_ops, 0.24, uv, wall_c, &mut reveals, reveal_c);
    }
    o.out.m(Mat::Trim).append(&reveals);

    // Ground floor parts.
    for (i, kind, op) in &ground_kinds {
        let cx = bay_x(*i);
        match kind {
            G::Window => {
                let sh = shutter_c.filter(|_| crate::hash01(wseed, 100 + *i as u64) < wd.shutters);
                bp::window(&mut o.out, b, op, wd, trim_c, sh, var.wall_mat);
            }
            G::Door => {
                bp::door(&mut o.out, b, op, door_c, trim_c);
                if var.ground == GroundFloor::Stoop {
                    bp::stoop(&mut o.out, b, cx, gy, op.y0, 1.7, lerp_color(wall_c, rgb(0x8a7a6a), 0.3));
                } else if op.y0 - gy > 0.1 {
                    // A couple of steps if the door sits above the street.
                    let st = Basis { o: b.o, ..*b };
                    mk::block(o.out.m(Mat::Paving), &st, Vec3::new(op.x0 - 0.15, gy - 0.2, 0.0), Vec3::new(op.x0 + op.w + 0.15, op.y0, 0.45), 0.02, rgb(0xbdb6aa));
                }
                if var.props.awning > 0.0 && crate::hash01(wseed, 7) < 0.3 {
                    bp::awning_at(&mut o.out, b, op.x0 - 0.2, op.w + 0.4, op.y0 + op.h + 0.35, 0.9, rgb(pick(&[0x1d3557, 0x9d0208, 0x2a2a2a], wseed, 8)));
                }
            }
            G::Shop | G::Arcade => {
                let sign = rgb(pick(&[0x1d3557, 0x9d0208, 0x2a2a2a, 0x264653, 0xe9c46a, 0x6a4c93, 0x2a9d8f], wseed, 30 + *i as u64));
                let aw = (crate::hash01(wseed, 40 + *i as u64) < var.props.awning).then(|| rgb(pick(&[0x2e7d6b, 0x355c7d, 0xb23a48, 0xe0a800, 0x1f3a5f, 0xc0392b], wseed, 41)));
                bp::shopfront(&mut o.out, b, op, scale_color(trim_c, 0.5), sign, if matches!(kind, G::Arcade) { None } else { aw }, wseed ^ *i as u64);
            }
            G::Garage => bp::garage_door(&mut o.out, b, op, trim_c, trim_c),
        }
    }

    // Upper floor parts.
    let continuous_balcony = var.balcony == Balcony::Iron && var.balcony_floors.len() >= 2 && building_balcony;
    for (k, i, op) in &upper_list {
        let sh = shutter_c.filter(|_| crate::hash01(wseed, 200 + (*k as u64) * 31 + *i as u64) < wd.shutters);
        bp::window(&mut o.out, b, op, wd, trim_c, sh, var.wall_mat);
        let r = crate::hash01(wseed, 300 + (*k as u64) * 37 + *i as u64);
        if var.props.flower_box > 0.0 && r < var.props.flower_box {
            bp::flower_box(&mut o.out, b, op.x0, op.w, op.y0, wseed ^ (*k as u64 * 97 + *i as u64));
        } else if var.props.ac_unit > 0.0 && r > 1.0 - var.props.ac_unit {
            if matches!(var.wall_mat, WallMat::Concrete) || var.window.w > 1.3 {
                bp::condenser(&mut o.out, b, op.x0 + op.w + 0.6f32.min(bay * 0.2), op.y0 - 0.1);
            } else {
                bp::ac_unit(&mut o.out, b, op.x0 + op.w * 0.5, op.y0 + 0.02);
            }
        }
        // Per-bay balconies.
        let on_floor = var.balcony_floors.contains(&(*k as u8)) || var.balcony_floors == [0];
        if !continuous_balcony && building_balcony && on_floor && var.balcony != Balcony::None {
            let bw = if var.balcony == Balcony::Juliet { op.w + 0.2 } else { (bay * 0.86).max(op.w + 0.4) };
            let fy = op.y0 - wd.sill_h;
            let x0 = op.x0 + op.w * 0.5 - bw * 0.5;
            bp::balcony(&mut o.out, b, var.balcony, x0, bw, if var.balcony == Balcony::Juliet { op.y0 } else { fy + 0.02 }, rgb(0x23262b), trim_c, wseed ^ *k as u64);
        }
    }
    // Continuous balconies (Haussmann 2nd and 5th floors).
    if continuous_balcony && nb > 0 {
        for &k in var.balcony_floors {
            if (k as u32) < floors && k >= 1 {
                let fy = floor_y(k as u32);
                bp::balcony(&mut o.out, b, Balcony::Iron, pier * 0.6, len - pier * 1.2, fy + 0.02, rgb(0x23262b), trim_c, wseed ^ k as u64);
            }
        }
    }
    // Fire escape on the front of tenements.
    if fire_escape && w.front && nb >= 3 {
        let (x0, x1) = (bay_x(0) - bay * 0.45, bay_x(1) + bay * 0.45);
        let ys: Vec<f32> = (1..floors).map(floor_y).collect();
        bp::fire_escape(&mut o.out, b, x0, x1, &ys);
    }
}

/// Little scroll brackets under a bracketed cornice.
fn brackets(o: &mut BuildingMeshes, walls: &[Wall], up: Vec3, y: f32, c: [f32; 4]) {
    for w in walls.iter().filter(|w| !w.party && w.len > 3.0) {
        let b = Basis::new(w.b, w.a - w.b, up);
        let n = (w.len / 1.1) as usize;
        for i in 0..=n {
            let x = 0.3 + (w.len - 0.6) * i as f32 / n.max(1) as f32;
            let prof = [Vec2::new(-0.06, 0.0), Vec2::new(0.06, 0.0), Vec2::new(0.06, 0.06), Vec2::new(-0.06, 0.06), Vec2::new(-0.06, 0.0)];
            let path = [b.p(x, y - 0.55, 0.02), b.p(x, y - 0.3, 0.2), b.p(x, y - 0.05, 0.45)];
            mk::sweep(o.out.m(Mat::Trim), &prof, &path, false, b.x, c);
        }
    }
}

/// Deterministic points inside a polygon (for rooftop props).
fn interior_points(poly: &[Vec3], up: Vec3, spacing: f32, seed: u64, max: usize) -> Vec<Vec3> {
    let ax = Vec3::X.reject_from_normalized(up).normalize_or(Vec3::X);
    let az = ax.cross(up);
    let pts2: Vec<Vec2> = poly.iter().map(|p| Vec2::new(p.dot(ax), p.dot(az))).collect();
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for p in &pts2 {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    let inside = |q: Vec2| {
        let mut c = false;
        for i in 0..pts2.len() {
            let (a, b) = (pts2[i], pts2[(i + 1) % pts2.len()]);
            if (a.y > q.y) != (b.y > q.y) && q.x < (b.x - a.x) * (q.y - a.y) / (b.y - a.y) + a.x {
                c = !c;
            }
        }
        c
    };
    let dist_edge = |q: Vec2| {
        (0..pts2.len())
            .map(|i| {
                let (a, b) = (pts2[i], pts2[(i + 1) % pts2.len()]);
                let t = ((q - a).dot(b - a) / (b - a).length_squared().max(1e-6)).clamp(0.0, 1.0);
                q.distance(a + (b - a) * t)
            })
            .fold(f32::MAX, f32::min)
    };
    let h0 = poly[0].dot(up);
    let mut out = Vec::new();
    let mut y = lo.y + spacing * 0.5;
    let mut k = 0u64;
    while y < hi.y && out.len() < max {
        let mut x = lo.x + spacing * (0.5 + 0.4 * crate::hash01(seed, k));
        while x < hi.x && out.len() < max {
            let q = Vec2::new(x, y);
            if inside(q) && dist_edge(q) > spacing * 0.45 {
                out.push(ax * q.x + az * q.y + up * h0);
            }
            x += spacing;
            k += 1;
        }
        y += spacing;
    }
    // Shuffle deterministically.
    out.sort_by(|a, b| crate::hash01(seed, (a.x * 100.0) as u64).total_cmp(&crate::hash01(seed, (b.x * 100.0) as u64)));
    out
}

#[allow(clippy::too_many_arguments)]
fn flat_roof(o: &mut BuildingMeshes, ring: &[Vec3], walls: &[Wall], up: Vec3, top: f32, var: &Variant, f: &Footprint, wall_c: [f32; 4], region: Region) {
    let parapet_h = if matches!(var.cornice, Cornice::None) { 0.55 } else { 0.9 };
    let deck_y = top - 0.05;
    // Parapet: outer faces continue the walls, inner faces inset.
    let inner = mk::inset(ring, up, 0.25);
    for (i, w) in walls.iter().enumerate() {
        let b = Basis::new(w.b, w.a - w.b, up);
        let mut rev = MeshData::default();
        mk::wall_with_openings(o.out.m(Mat::Wall(var.wall_mat)), &b, w.len, top, top + parapet_h, &[], 0.0, 2.0, wall_c, &mut rev, wall_c);
        let (ia, ib) = (inner[i], inner[(i + 1) % inner.len()]);
        let p = [ib + up * deck_y, ia + up * deck_y, ia + up * (top + parapet_h), ib + up * (top + parapet_h)];
        mk::m_quad_facing(o.out.m(Mat::Wall(var.wall_mat)), p, (ia + ib) * 0.5 - w.outward + up * deck_y, scale_color(wall_c, 0.9));
    }
    // Coping along the parapet top.
    let mid: Vec<Vec3> = ring.iter().zip(&inner).map(|(a, b)| (*a + *b) * 0.5 + up * (top + parapet_h)).collect();
    mk::sweep(o.out.m(Mat::Trim), &profiles::coping(0.3, 0.1), &mid, true, up, rgb(0xb8b2a8));
    // Roof deck.
    let deck: Vec<Vec3> = inner.iter().map(|p| *p + up * deck_y).collect();
    mk::polygon(o.out.m(Mat::Roof(RoofMat::Gravel)), &deck, &[], up, 2.0, rgb(pick(var.roofs, f.seed, 4)));

    // Rooftop gear.
    let area = mk::signed_area(&inner, up);
    if area < 15.0 {
        return;
    }
    let spots = interior_points(&deck, up, 3.2, f.seed, 24);
    let mut s = spots.into_iter();
    let x_axis = (walls[0].a - walls[0].b).reject_from_normalized(up).normalize_or(Vec3::X);
    let basis_at = |p: Vec3| Basis::new(p, x_axis, up);
    let r = |k: u64| crate::hash01(f.seed, 500 + k);
    // Stair bulkhead on taller buildings.
    if area > 60.0 && top > 9.0 {
        if let Some(p) = s.next() {
            bp::bulkhead(&mut o.out, &basis_at(p), scale_color(wall_c, 0.95), var.wall_mat);
        }
    }
    if r(1) < var.props.water_tank {
        if let Some(p) = s.next() {
            if region == Region::NorthAmericaCity {
                bp::water_tank_nyc(&mut o.out, p, up);
            } else {
                bp::water_tank_poly(&mut o.out, p, up);
                if let Some(p2) = s.next().filter(|_| r(2) < 0.5) {
                    bp::water_tank_poly(&mut o.out, p2, up);
                }
            }
        }
    }
    if r(3) < var.props.rooftop_garden && area > 40.0 {
        if let Some(p) = s.next() {
            bp::rooftop_garden(&mut o.out, &basis_at(p), 5.5f32.min(area.sqrt() * 0.7), 5.0, f.seed);
        }
    }
    if r(4) < var.props.solar {
        if let Some(p) = s.next() {
            bp::solar_array(&mut o.out, &basis_at(p), 3, 2, 0.45);
        }
    }
    let dishes = if r(5) < var.props.satellite { 1 + (r(6) * 3.0) as usize } else { 0 };
    for k in 0..dishes {
        if let Some(p) = s.next() {
            let aim = (x_axis * (r(10 + k as u64) - 0.5) + up.any_orthonormal_vector() * 0.3 + up * 0.6).normalize();
            bp::satellite_dish(&mut o.out, p + up * 0.9, aim, up, 0.35 + 0.15 * r(20 + k as u64));
        }
    }
    let hvacs = if top > 10.0 && area > 80.0 { 1 + (r(7) * 3.0) as usize } else if area > 50.0 && r(8) < 0.4 { 1 } else { 0 };
    for _ in 0..hvacs {
        if let Some(p) = s.next() {
            bp::hvac(&mut o.out, &basis_at(p), 1.0 + r(9) * 0.6);
        }
    }
    if region == Region::SouthAsia {
        // Rebar stubs from columns waiting for the next floor.
        for (i, p) in inner.iter().enumerate().take(6) {
            for k in 0..4 {
                let off = Vec3::new((k % 2) as f32 * 0.15, 0.0, (k / 2) as f32 * 0.15);
                bp::bar(&mut o.out, Mat::Metal, *p + up * deck_y + off, *p + up * (deck_y + 0.9 + 0.2 * crate::hash01(f.seed, i as u64)) + off, 0.016, Vec3::X, rgb(0x6b4a3a));
            }
        }
    }
}

/// Oriented bounding box in the roof plane.
struct Obb {
    center: Vec3,
    axis_l: Vec3,
    axis_s: Vec3,
    half: Vec2,
    rectangularity: f32,
}

fn fit_obb(ring: &[Vec3], up: Vec3) -> Option<Obb> {
    let area = mk::signed_area(ring, up).abs();
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
        let bb = up.cross(a).normalize();
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for p in ring {
            let q = Vec2::new(p.dot(a), p.dot(bb));
            lo = lo.min(q);
            hi = hi.max(q);
        }
        let box_area = (hi - lo).x * (hi - lo).y;
        if best.as_ref().is_none_or(|b| box_area < b.0) {
            best = Some((box_area, a, bb, lo, hi));
        }
    }
    let (box_area, a, bb, lo, hi) = best?;
    let mid = (lo + hi) * 0.5;
    let height = ring.iter().map(|p| p.dot(up)).sum::<f32>() / ring.len() as f32;
    let center = a * mid.x + bb * mid.y + up * height;
    let ext = (hi - lo) * 0.5;
    let (axis_l, axis_s, half) = if ext.x >= ext.y { (a, bb, ext) } else { (bb, a, Vec2::new(ext.y, ext.x)) };
    Some(Obb { center, axis_l, axis_s, half, rectangularity: area / box_area })
}

/// A roof plane with tile courses: the main surface plus a small step every
/// course so tiles/shingles cast shadow lines.
fn roof_plane(o: &mut BuildingMeshes, mat: RoofMat, eave_a: Vec3, eave_b: Vec3, ridge_b: Vec3, ridge_a: Vec3, c: [f32; 4]) {
    let course = match mat {
        RoofMat::ClayTile => 0.32,
        RoofMat::Slate | RoofMat::Shingle => 0.25,
        RoofMat::Metal | RoofMat::Zinc => 0.0,
        RoofMat::Gravel => 0.0,
    };
    let slope_len = ((eave_a + eave_b) * 0.5).distance((ridge_a + ridge_b) * 0.5);
    let along = (eave_b - eave_a).normalize_or(Vec3::X);
    let up_slope = ((ridge_a + ridge_b) * 0.5 - (eave_a + eave_b) * 0.5).normalize_or(Vec3::Y);
    let normal = along.cross(up_slope).normalize_or(Vec3::Y);
    let m = o.out.m(Mat::Roof(mat));
    if course <= 0.0 || slope_len < 0.6 {
        let uvs = [[0.0, 0.0], [eave_a.distance(eave_b), 0.0], [ridge_a.distance(ridge_b), slope_len], [0.0, slope_len]];
        m.quad([eave_a, eave_b, ridge_b, ridge_a], uvs, c);
        return;
    }
    let n = (slope_len / course).ceil() as usize;
    for k in 0..n {
        let t0 = k as f32 / n as f32;
        let t1 = (k + 1) as f32 / n as f32;
        let a0 = eave_a.lerp(ridge_a, t0);
        let b0 = eave_b.lerp(ridge_b, t0);
        let a1 = eave_a.lerp(ridge_a, t1);
        let b1 = eave_b.lerp(ridge_b, t1);
        // Lift the lower edge of each course slightly: overlapping tiles.
        let lift = normal * 0.025;
        let shade = 0.92 + 0.08 * ((k * 7919) % 13) as f32 / 13.0;
        let col = scale_color(c, shade);
        let len0 = a0.distance(b0);
        let len1 = a1.distance(b1);
        m.quad([a0 + lift, b0 + lift, b1, a1], [[0.0, 0.0], [len0, 0.0], [len1, 1.0], [0.0, 1.0]], col);
        // Butt edge of the course.
        m.quad([a0, b0, b0 + lift, a0 + lift], [[0.0, 0.0], [len0, 0.0], [len0, 0.05], [0.0, 0.05]], scale_color(col, 0.7));
    }
}

#[allow(clippy::too_many_arguments)]
fn pitched(o: &mut BuildingMeshes, ring: &[Vec3], walls: &[Wall], up: Vec3, top: f32, form: RoofForm, var: &Variant, f: &Footprint, roof_c: [f32; 4], trim_c: [f32; 4], wall_c: [f32; 4]) {
    let eave_ring: Vec<Vec3> = ring.iter().map(|p| *p + up * top).collect();
    let rect = fit_obb(&eave_ring, up).filter(|b| b.rectangularity > 0.82 && ring.len() <= 12 && b.half.y > 1.5);
    let pitch = if form == RoofForm::LowHip { var.pitch.min(0.4) } else { var.pitch };
    let ov = var.overhang;
    let Some(obb) = rect else {
        // Any footprint: hip roof by insetting the eave ring.
        let span = (2.0 * mk::signed_area(&eave_ring, up) / perimeter(&eave_ring)).max(1.0);
        let d = span * 0.95;
        let rise = (pitch * d).min(6.0);
        let outer = mk::inset(&eave_ring, up, -ov);
        let drop = rise * ov / d.max(0.1);
        let outer: Vec<Vec3> = outer.iter().map(|p| *p - up * drop).collect();
        let inner: Vec<Vec3> = mk::inset(&eave_ring, up, d).iter().map(|p| *p + up * rise).collect();
        for i in 0..outer.len() {
            let j = (i + 1) % outer.len();
            roof_plane(o, var.roof_mat, outer[i], outer[j], inner[j], inner[i], roof_c);
        }
        mk::polygon(o.out.m(Mat::Roof(var.roof_mat)), &inner, &[], up, 1.0, roof_c);
        fascia_and_soffit(o, &outer, &eave_ring, up, trim_c);
        roof_extras(o, &obb_from_ring(&eave_ring, up), up, top + rise * 0.6, var, f, wall_c);
        return;
    };
    let (l, s, c) = (obb.axis_l, obb.axis_s, obb.center);
    let (hl, hs) = (obb.half.x, obb.half.y);
    let rise = (pitch * hs).min(7.0);
    let drop = rise * ov / hs;
    let e = |sl: f32, ss: f32| c + l * sl * (hl + if form == RoofForm::Gable { ov * 0.6 } else { ov }) + s * ss * (hs + ov) - up * drop;
    match form {
        RoofForm::Gable => {
            let ridge = |sl: f32| c + l * sl * (hl + ov * 0.6) + up * rise;
            roof_plane(o, var.roof_mat, e(1.0, 1.0), e(-1.0, 1.0), ridge(-1.0), ridge(1.0), roof_c);
            roof_plane(o, var.roof_mat, e(-1.0, -1.0), e(1.0, -1.0), ridge(1.0), ridge(-1.0), scale_color(roof_c, 0.9));
            // Gable end walls in the wall material, at the wall line.
            for sl in [1.0f32, -1.0] {
                let g0 = c + l * sl * hl + s * hs;
                let g1 = c + l * sl * hl - s * hs;
                let apex = c + l * sl * hl + up * rise;
                let mm = o.out.m(Mat::Wall(var.wall_mat));
                let n = (g1 - g0).cross(apex - g0);
                if n.dot(l * sl) >= 0.0 { mm.triangle([g0, g1, apex], [[0.0, 0.0], [hs, 0.0], [hs * 0.5, rise]], wall_c) } else { mm.triangle([g1, g0, apex], [[0.0, 0.0], [hs, 0.0], [hs * 0.5, rise]], wall_c) }
                // Bargeboards along the rakes.
                for ss in [1.0f32, -1.0] {
                    let rake = [e(sl, ss), ridge(sl)];
                    bp::bar(&mut o.out, Mat::Trim, rake[0] + l * sl * 0.03, rake[1] + l * sl * 0.03, 0.12, l, trim_c);
                }
                // Gable vent.
                let vb = Basis::new(c + l * sl * (hl + 0.01) + up * (rise * 0.35), s * -sl, up);
                for k in 0..5 {
                    let y = k as f32 * 0.09;
                    mk::block(o.out.m(Mat::Trim), &vb, Vec3::new(-0.3, y, 0.0), Vec3::new(0.3, y + 0.05, 0.05), 0.0, trim_c);
                }
            }
            // Ridge cap.
            let rp = [c + l * (hl + ov * 0.6) + up * rise, c - l * (hl + ov * 0.6) + up * rise];
            mk::sweep(o.out.m(Mat::Roof(var.roof_mat)), &[Vec2::new(-0.14, -0.06), Vec2::new(0.0, 0.08), Vec2::new(0.14, -0.06)], &rp, false, up, scale_color(roof_c, 0.85));
            let eaves = [e(1.0, 1.0), e(-1.0, 1.0)];
            bp::bar(&mut o.out, Mat::Trim, eaves[0] - up * 0.08, eaves[1] - up * 0.08, 0.16, up, trim_c);
            let eaves = [e(1.0, -1.0), e(-1.0, -1.0)];
            bp::bar(&mut o.out, Mat::Trim, eaves[0] - up * 0.08, eaves[1] - up * 0.08, 0.16, up, trim_c);
        }
        _ => {
            let ridge_half = (hl - hs).max(0.0);
            let r0 = c + l * ridge_half + up * rise;
            let r1 = c - l * ridge_half + up * rise;
            roof_plane(o, var.roof_mat, e(1.0, 1.0), e(-1.0, 1.0), r1, r0, roof_c);
            roof_plane(o, var.roof_mat, e(-1.0, -1.0), e(1.0, -1.0), r0, r1, scale_color(roof_c, 0.9));
            roof_plane(o, var.roof_mat, e(1.0, -1.0), e(1.0, 1.0), r0, r0, scale_color(roof_c, 0.95));
            roof_plane(o, var.roof_mat, e(-1.0, 1.0), e(-1.0, -1.0), r1, r1, scale_color(roof_c, 0.85));
            let hip_c = scale_color(roof_c, 0.82);
            let cap = [Vec2::new(-0.12, -0.05), Vec2::new(0.0, 0.07), Vec2::new(0.12, -0.05)];
            mk::sweep(o.out.m(Mat::Roof(var.roof_mat)), &cap, &[r0, r1], false, up, hip_c);
            for (a, rr) in [(e(1.0, 1.0), r0), (e(1.0, -1.0), r0), (e(-1.0, 1.0), r1), (e(-1.0, -1.0), r1)] {
                mk::sweep(o.out.m(Mat::Roof(var.roof_mat)), &cap, &[a, rr], false, up, hip_c);
            }
            let outer = [e(1.0, 1.0), e(-1.0, 1.0), e(-1.0, -1.0), e(1.0, -1.0)];
            fascia_and_soffit(o, &outer, &eave_ring, up, trim_c);
        }
    }
    roof_extras(o, &obb, up, top + rise * 0.55, var, f, wall_c);
    let _ = walls;
}

fn perimeter(ring: &[Vec3]) -> f32 {
    (0..ring.len()).map(|i| ring[i].distance(ring[(i + 1) % ring.len()])).sum::<f32>().max(1e-3)
}

fn obb_from_ring(ring: &[Vec3], up: Vec3) -> Obb {
    fit_obb(ring, up).unwrap_or(Obb { center: ring[0], axis_l: Vec3::X, axis_s: Vec3::Z, half: Vec2::splat(1.0), rectangularity: 1.0 })
}

/// Fascia board around the eaves and a soffit back to the wall.
fn fascia_and_soffit(o: &mut BuildingMeshes, outer: &[Vec3], eave_ring: &[Vec3], up: Vec3, trim: [f32; 4]) {
    let n = outer.len();
    for i in 0..n {
        let j = (i + 1) % n;
        bp::bar(&mut o.out, Mat::Trim, outer[i] - up * 0.09, outer[j] - up * 0.09, 0.18, up, trim);
    }
    if eave_ring.len() == n {
        for i in 0..n {
            let j = (i + 1) % n;
            let p = [eave_ring[i] - up * 0.02, eave_ring[j] - up * 0.02, outer[j] - up * 0.16, outer[i] - up * 0.16];
            mk::m_quad_facing(o.out.m(Mat::Trim), p, (p[0] + p[2]) * 0.5 - up, scale_color(trim, 0.85));
        }
    }
}

/// Chimneys (and a few solar panels) on pitched roofs.
fn roof_extras(o: &mut BuildingMeshes, obb: &Obb, up: Vec3, y: f32, var: &Variant, f: &Footprint, wall_c: [f32; 4]) {
    if crate::hash01(f.seed, 41) < var.props.chimney {
        let side = if crate::hash01(f.seed, 42) < 0.5 { 1.0 } else { -1.0 };
        let base = obb.center.reject_from_normalized(up) + up * y + obb.axis_l * side * obb.half.x * 0.55 + obb.axis_s * obb.half.y * 0.2;
        let brick = if var.wall_mat == WallMat::Brick { wall_c } else { rgb(0x9e4f3c) };
        bp::chimney(&mut o.out, base, up, obb.axis_l, 1.6, 1 + (crate::hash01(f.seed, 43) * 3.0) as usize, brick);
    }
}

#[allow(clippy::too_many_arguments)]
fn mansard(o: &mut BuildingMeshes, ring: &[Vec3], walls: &[Wall], up: Vec3, top: f32, var: &Variant, f: &Footprint, roof_c: [f32; 4], trim_c: [f32; 4], wall_c: [f32; 4]) {
    let lower_h = 2.9;
    let inset = 1.0;
    let base_ring: Vec<Vec3> = ring.iter().map(|p| *p + up * (top + 0.05)).collect();
    let mid_ring: Vec<Vec3> = mk::inset(ring, up, inset).iter().map(|p| *p + up * (top + lower_h)).collect();
    // Lower steep slope (zinc), then a shallow upper hip.
    for i in 0..ring.len() {
        let j = (i + 1) % ring.len();
        roof_plane(o, RoofMat::Zinc, base_ring[i], base_ring[j], mid_ring[j], mid_ring[i], roof_c);
    }
    let span = (2.0 * mk::signed_area(&mid_ring, up) / perimeter(&mid_ring)).max(0.5);
    let up_rise = (span * 0.3).min(2.0);
    let top_ring: Vec<Vec3> = mk::inset(&mid_ring, up, span * 0.9).iter().map(|p| *p + up * up_rise).collect();
    for i in 0..ring.len() {
        let j = (i + 1) % ring.len();
        roof_plane(o, RoofMat::Zinc, mid_ring[i], mid_ring[j], top_ring[j], top_ring[i], scale_color(roof_c, 0.9));
    }
    mk::polygon(o.out.m(Mat::Roof(RoofMat::Zinc)), &top_ring, &[], up, 1.0, roof_c);
    // Break line trim between the two slopes.
    mk::sweep(o.out.m(Mat::Trim), &[Vec2::new(-0.06, 0.0), Vec2::new(0.06, 0.0), Vec2::new(0.06, 0.08), Vec2::new(-0.06, 0.08), Vec2::new(-0.06, 0.0)], &mid_ring, true, up, rgb(0x5f6b74));
    // Dormers on street walls, aligned with the window bays below.
    for w in walls.iter().filter(|w| !w.party && w.len > 3.0) {
        let pier_min = 0.35;
        let nb = (((w.len - 2.0 * pier_min) / var.bay_w).floor() as i32).max(1) as usize;
        let bay = var.bay_w.min((w.len - 2.0 * pier_min) / nb as f32).max(1.2);
        let pier = (w.len - nb as f32 * bay) * 0.5;
        let b = Basis::new(w.b, w.a - w.b, up);
        for i in 0..nb {
            if i % 2 == 1 && nb > 3 {
                continue; // every other bay, like real Haussmann attics
            }
            let cx = pier + (i as f32 + 0.5) * bay;
            dormer(o, &b, cx, top + 0.25, -0.35, var, trim_c, roof_c, wall_c, f.seed ^ i as u64);
        }
    }
    // Party-wall chimney stacks with rows of pots.
    for w in walls.iter().filter(|w| w.party && w.len > 4.0) {
        let mid = (w.a + w.b) * 0.5 - w.outward * 0.5 + up * (top + lower_h - 0.3);
        let along = (w.a - w.b).normalize();
        bp::chimney(&mut o.out, mid, up, along, 1.4, 4 + (crate::hash01(f.seed, 60) * 3.0) as usize, rgb(0xd9cdb5));
    }
}

#[allow(clippy::too_many_arguments)]
fn dormer(o: &mut BuildingMeshes, b: &Basis, cx: f32, y0: f32, z: f32, var: &Variant, trim_c: [f32; 4], roof_c: [f32; 4], wall_c: [f32; 4], seed: u64) {
    let (w, h) = (1.15, 1.75);
    let face = b.at(0.0, 0.0, z);
    let op = Opening { x0: cx - 0.4, y0: y0 + 0.3, w: 0.8, h: 1.2, arch: Arch::Segmental(0.18) };
    let mut rev = MeshData::default();
    // Front panel with the window hole (stone like the facade).
    let fb = face.at(cx - w * 0.5, 0.0, 0.0);
    let local_op = Opening { x0: op.x0 - (cx - w * 0.5), ..op };
    mk::wall_with_openings(o.out.m(Mat::Wall(WallMat::Stone)), &fb, w, y0, y0 + h, &[local_op], 0.18, 2.0, wall_c, &mut rev, scale_color(wall_c, 0.8));
    o.out.m(Mat::Trim).append(&rev);
    let wd = archkit::WindowDesign { shutters: 0.0, grille: false, surround: true, lintel: archkit::Lintel::None, ..var.window };
    bp::window(&mut o.out, &fb, &local_op, &wd, trim_c, None, WallMat::Stone);
    // Cheeks back into the roof and a little curved/pitched cap.
    let depth = 1.3;
    for x in [cx - w * 0.5, cx + w * 0.5] {
        let p = [face.p(x, y0, 0.0), face.p(x, y0 + h, 0.0), face.p(x, y0 + h, -depth), face.p(x, y0 + h * 0.15, -depth)];
        mk::m_quad_facing(o.out.m(Mat::Roof(RoofMat::Zinc)), p, face.p(x + (x - cx) * 4.0, y0 + h * 0.5, -depth * 0.5), scale_color(roof_c, 0.85));
    }
    let cap_prof: Vec<Vec2> = (0..=8).map(|k| {
        let a = k as f32 / 8.0 * std::f32::consts::PI;
        Vec2::new((w * 0.5 + 0.1) * a.cos(), 0.28 * a.sin())
    }).collect();
    let mut prof = cap_prof.clone();
    prof.reverse();
    mk::sweep(o.out.m(Mat::Roof(RoofMat::Zinc)), &prof, &[face.p(cx, y0 + h, 0.12), face.p(cx, y0 + h, -depth)], false, b.y, roof_c);
    let _ = seed;
}

/// Detached-house lot: porch, driveway to the garage, walkway, mailbox, fence.
#[allow(clippy::too_many_arguments)]
fn lot(o: &mut BuildingMeshes, walls: &[Wall], up: Vec3, gy: f32, var: &Variant, f: &Footprint, ctx: &DetailCtx, trim_c: [f32; 4], roof_c: [f32; 4], door_c: [f32; 4]) {
    let Some(front) = walls.iter().find(|w| w.front) else { return };
    let b = Basis::new(front.b, front.a - front.b, up);
    let mid = (front.a + front.b) * 0.5;
    let road = nearest_road(mid, &ctx.roads, up);
    let to_road = road.map(|r| r.0).unwrap_or(8.0).clamp(3.0, 30.0);
    let pier_min = 0.35;
    let nb = (((front.len - 2.0 * pier_min) / var.bay_w).floor() as i32).max(1) as usize;
    let bay = var.bay_w.min((front.len - 2.0 * pier_min) / nb as f32).max(1.2);
    let pier = (front.len - nb as f32 * bay) * 0.5;
    let door_x = pier + (nb / 2) as f32 * bay + bay * 0.5;
    let porch_odds = match var.name {
        "Colonial" | "Craftsman" | "Weatherboard house" => 0.75,
        _ => 0.25,
    };
    let ground = |p: Vec3| -> Vec3 {
        // Snap a local point to the terrain surface.
        let g = ctx.proj.frame.render_to_geodetic(p.as_dvec3() + ctx.proj.anchor);
        let (u, v) = ctx.proj.tile.geodetic_to_uv(g.lat_deg, g.lon_deg);
        ctx.proj.local(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0), ctx.grid.sample(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)) as f64 + 0.06)
    };
    if crate::hash01(f.seed, 70) < porch_odds {
        let pw = (front.len * 0.45).clamp(3.0, 7.0);
        bp::porch(&mut o.out, &b.at(0.0, gy, 0.0), door_x, pw, 2.2, 2.5, trim_c, roof_c, var.roof_mat);
    }
    // Walkway from the door (or porch steps) to the road.
    let start = b.p(door_x, gy, 2.8);
    let end = start + front.outward * (to_road - 3.0).max(0.5);
    paving_strip(o, start, end, 1.2, up, rgb(0xc9c3b8), &ground);
    // Driveway from the garage bay.
    if nb >= 3 && front.len > 8.5 {
        let gx = pier + (nb - 1) as f32 * bay + bay * 0.5;
        let s = b.p(gx, gy, 0.1);
        let e = s + front.outward * to_road.max(4.0);
        paving_strip(o, s, e, 3.2, up, rgb(0xb8b3aa), &ground);
        let mb = ground(e - front.outward * 1.0 + (front.a - front.b).normalize() * 2.2);
        bp::mailbox(&mut o.out, mb, up, front.outward, door_c);
    } else {
        let mb = ground(end - front.outward * 0.6 + (front.a - front.b).normalize() * 1.0);
        bp::mailbox(&mut o.out, mb, up, front.outward, door_c);
    }
    // Picket fence along the front yard edge with a gap for the walkway.
    if crate::hash01(f.seed, 71) < 0.35 && to_road > 6.0 {
        let along = (front.a - front.b).normalize();
        let line_o = mid + front.outward * (to_road - 2.5);
        let half = front.len * 0.5 + 2.0;
        let gap_c = start + front.outward * (to_road - 2.5 - 2.8);
        let gap_t = (gap_c - line_o).dot(along);
        let seg1 = [ground(line_o - along * half), ground(line_o + along * (gap_t - 0.7))];
        let seg2 = [ground(line_o + along * (gap_t + 0.7)), ground(line_o + along * half)];
        bp::picket_fence(&mut o.out, &seg1, up, rgb(0xf4f2ec));
        bp::picket_fence(&mut o.out, &seg2, up, rgb(0xf4f2ec));
    }
}

fn paving_strip(o: &mut BuildingMeshes, a: Vec3, b: Vec3, width: f32, up: Vec3, c: [f32; 4], ground: &dyn Fn(Vec3) -> Vec3) {
    let d = (b - a).reject_from_normalized(up);
    let len = d.length();
    if len < 0.3 {
        return;
    }
    let side = d.normalize().cross(up) * (width * 0.5);
    let n = (len / 2.0).ceil() as usize;
    let m = o.out.m(Mat::Paving);
    for k in 0..n {
        let p0 = a + d * (k as f32 / n as f32);
        let p1 = a + d * ((k + 1) as f32 / n as f32);
        let q = [ground(p0 - side), ground(p0 + side), ground(p1 + side), ground(p1 - side)];
        let nrm = (q[1] - q[0]).cross(q[2] - q[0]);
        if nrm.dot(up) >= 0.0 { m.quad(q, [[0.0; 2]; 4], c) } else { m.quad([q[1], q[0], q[3], q[2]], [[0.0; 2]; 4], c) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_geo::{EnuFrame, Geodetic, tile::TileId};

    fn tile_with(rings: Vec<Vec<[f32; 2]>>, height: Option<f64>) -> mvt::Tile {
        let features = rings
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                let mut props = HashMap::new();
                if let Some(h) = height {
                    props.insert("render_height".to_string(), mvt::Value::Double(h));
                }
                mvt::Feature { id: Some(i as u64 + 1), geom_type: GeomType::Polygon, props, rings: vec![r] }
            })
            .collect();
        mvt::Tile { layers: vec![mvt::Layer { name: "building".into(), extent: 4096, features }] }
    }

    fn rect(x0: f32, y0: f32, w: f32, h: f32) -> Vec<[f32; 2]> {
        // Clockwise in tile space (y down) = positive area in MVT.
        vec![[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]]
    }

    fn run(region: Region, vt: &mvt::Tile) -> BuildingMeshes {
        let (lat, lon) = (40.0, -3.7);
        let tile = TileId::containing(lat, lon, 14);
        let proj = TileProjector::new(tile, EnuFrame::new(Geodetic::new(lat, lon, 0.0)));
        let grid = TerrainGrid::flat(tile, 8, 100.0);
        let mut surface = SurfaceMap::uniform(tile, Biome::Mediterranean);
        surface.cover = vec![Cover::Residential];
        let road = vec![(RoadClass::Secondary, vec![proj.local(0.40, 0.495, 100.0), proj.local(0.60, 0.495, 100.0)])];
        generate_in_region(vt, &BuildingContext { proj: &proj, grid: &grid, surface: &surface, biome: Biome::Mediterranean, roads: &road }, region)
    }

    #[test]
    fn every_region_builds_detailed_buildings() {
        let vt = tile_with(vec![rect(2000.0, 2040.0, 40.0, 30.0)], None);
        for r in Region::ALL {
            let m = run(r, &vt);
            assert_eq!(m.count, 1, "{r:?}");
            let t = m.triangle_count();
            assert!(t > 600, "{r:?}: only {t} triangles");
            assert!(m.out.meshes.contains_key(&Mat::Glass), "{r:?} has glass");
            for mesh in m.out.meshes.values() {
                assert!(mesh.positions.iter().all(|p| p.iter().all(|c| c.is_finite())), "{r:?}");
            }
        }
    }

    #[test]
    fn party_walls_have_no_windows() {
        // Two buildings sharing an edge vs. the same building alone.
        let alone = run(Region::Paris, &tile_with(vec![rect(2000.0, 2040.0, 40.0, 30.0)], Some(20.0)));
        let pair = run(Region::Paris, &tile_with(vec![rect(2000.0, 2040.0, 40.0, 30.0), rect(2040.0, 2040.0, 40.0, 30.0)], Some(20.0)));
        let glass = |m: &BuildingMeshes| m.out.meshes.get(&Mat::Glass).map_or(0, |g| g.triangle_count());
        assert!(glass(&pair) < glass(&alone) * 2, "shared wall should drop windows: {} vs 2x{}", glass(&pair), glass(&alone));
    }

    #[test]
    fn heights_snap_to_floors() {
        let vt = tile_with(vec![rect(2000.0, 2040.0, 40.0, 40.0)], Some(18.0));
        let (lat, lon) = (40.0, -3.7);
        let tile = TileId::containing(lat, lon, 14);
        let proj = TileProjector::new(tile, EnuFrame::new(Geodetic::new(lat, lon, 0.0)));
        let grid = TerrainGrid::flat(tile, 8, 100.0);
        let surface = SurfaceMap::uniform(tile, Biome::TemperateForest);
        let ctx = BuildingContext { proj: &proj, grid: &grid, surface: &surface, biome: Biome::TemperateForest, roads: &[] };
        let specs = prepare(&vt, &ctx, Region::NorthAmericaCity);
        let m = build_lod(specs.iter(), proj.up());
        assert_eq!(m.count, 1);
        // Collider top = wall top: ground 3.9 + floors*3.1 ...
        let ys: Vec<f32> = m.collider.positions.iter().map(|p| p[1]).collect();
        let span = ys.iter().cloned().fold(f32::MIN, f32::max) - ys.iter().cloned().fold(f32::MAX, f32::min);
        assert!((14.0..22.0).contains(&span), "{span}");
    }
}
