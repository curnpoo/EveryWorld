//! Lookdev: ground v2 board. Lives in its own file so the ground work stays
//! out of everyone else's way in `lookdev.rs`.
//!
//! Builds:
//! - the big lookdev field (textured grass) the street vignettes stand on,
//! - a swatch board: every ground layer pure, grass under every biome tint,
//!   and the common two-layer mixes,
//! - six small "places" on their own z15 tiles around the main tile, each a
//!   3x3 block with its own relief, land cover and scatter: meadow, forest
//!   floor, desert sand, alpine rock + snow, Mediterranean dry scrub and an
//!   urban park edge. Each gets a standing (1.5 m), crouched (0.4 m) and
//!   aerial shot.

use super::Shot;
use crate::assets::WorldMaterials;
use crate::ground_scatter::GroundCover;
use crate::meshconv::to_mesh;
use crate::origin::{GeoAnchored, WorldOrigin};
use bevy::prelude::*;
use ed_data::mvt::{self, GeomType, Value};
use ed_geo::tile::TileId;
use ed_world::biome::Biome;
use ed_world::buildings::{self, BuildingContext};
use ed_world::foliage::{self, Occupancy};
use ed_world::ground_scatter::GroundSource;
use ed_world::mesh::MeshData;
use ed_world::noise::{fbm, smoothstep};
use ed_world::project::TileProjector;
use ed_world::style::Region;
use ed_world::surface::{Cover, Ground, LAYERS, SurfaceMap};
use ed_world::terrain::{self, TerrainGrid};
use ed_world::{props, roads};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Place {
    Meadow,
    Forest,
    Desert,
    Alpine,
    Mediterranean,
    UrbanPark,
}

impl Place {
    fn name(self) -> &'static str {
        match self {
            Place::Meadow => "meadow",
            Place::Forest => "forest_floor",
            Place::Desert => "desert_sand",
            Place::Alpine => "alpine_rock_snow",
            Place::Mediterranean => "mediterranean_scrub",
            Place::UrbanPark => "urban_park_edge",
        }
    }

    fn biome(self) -> Biome {
        match self {
            Place::Meadow | Place::Forest | Place::UrbanPark => Biome::TemperateForest,
            Place::Desert => Biome::HotDesert,
            Place::Alpine => Biome::Alpine,
            Place::Mediterranean => Biome::Mediterranean,
        }
    }

    /// z15 offset of the block center from the main tile's first child.
    fn offset(self) -> (i64, i64) {
        match self {
            Place::Alpine => (-5, -2),
            Place::Mediterranean => (-2, -2),
            Place::Desert => (1, -2),
            Place::Meadow => (4, -2),
            Place::Forest => (-2, 1),
            Place::UrbanPark => (4, 1),
        }
    }

    /// Relief added on top of the shared base, at global z15 coords.
    fn relief(self, gx: f64, gy: f64) -> f32 {
        match self {
            Place::Meadow => 18.0 * fbm(gx * 2.5, gy * 2.5, 41, 4, 0),
            Place::Forest => 35.0 * fbm(gx * 2.0, gy * 2.0, 42, 4, 0),
            Place::Desert => {
                // Long dune crests with a soft stoss side.
                let w = fbm(gx * 1.5, gy * 1.5, 43, 3, 0) as f64 * 1.4;
                let ph = ((gy * 9.0 + w).fract()) as f32;
                let dune = if ph < 0.7 { ph / 0.7 } else { (1.0 - ph) / 0.3 };
                14.0 * smoothstep(0.0, 1.0, dune) + 10.0 * fbm(gx * 3.0, gy * 3.0, 44, 3, 0)
            }
            Place::Alpine => 0.0, // handled by the mountain shape below
            Place::Mediterranean => 90.0 * fbm(gx * 3.0, gy * 3.0, 45, 5, 0),
            Place::UrbanPark => 1.5 * fbm(gx * 6.0, gy * 6.0, 46, 2, 0),
        }
    }

    fn cover(self, u: f64, v: f64, gx: f64, gy: f64, h: f32, slope: f32) -> Cover {
        let n = fbm(gx * 18.0, gy * 18.0, 50, 4, 0);
        let m = fbm(gx * 40.0, gy * 40.0, 51, 3, 0);
        match self {
            Place::Meadow => {
                if n > 0.66 { Cover::Wood } else if m > 0.7 { Cover::Natural } else { Cover::Grass }
            }
            Place::Forest => {
                if n < 0.3 { Cover::Grass } else { Cover::Wood }
            }
            Place::Desert => {
                if m > 0.72 { Cover::Rock } else if n < 0.38 { Cover::Natural } else { Cover::Sand }
            }
            Place::Alpine => {
                if h > 1650.0 + 120.0 * m && slope < 0.35 { Cover::Ice } else if slope > 0.3 || m > 0.68 { Cover::Rock } else { Cover::Natural }
            }
            Place::Mediterranean => {
                if n > 0.68 { Cover::Wood } else { Cover::Natural }
            }
            Place::UrbanPark => {
                // West: park; east: residential block across a street.
                let _ = v;
                if u < 0.52 { Cover::Park } else { Cover::Residential }
            }
        }
    }
}

const PLACES: [Place; 6] = [Place::Meadow, Place::Forest, Place::Desert, Place::Alpine, Place::Mediterranean, Place::UrbanPark];

/// Shared gentle base height for the whole lookdev area (m).
fn base_height(gx: f64, gy: f64) -> f32 {
    120.0 + 25.0 * fbm(gx * 0.8, gy * 0.8, 40, 3, 0)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &WorldMaterials,
    origin: &WorldOrigin,
    tile: TileId,
    proj: &TileProjector,
    grid: &TerrainGrid,
    ground_h: f64,
) -> Vec<Shot> {
    let mut shots = Vec::new();
    let anchor_v = proj.anchor.as_vec3();
    let spawn_root = |commands: &mut Commands, p: &TileProjector, name: String| -> Entity {
        let anchor = GeoAnchored::from_render(&origin.frame, p.anchor);
        commands.spawn((anchor, Transform::default(), Visibility::default(), Name::new(name))).id()
    };
    let add = |commands: &mut Commands, meshes: &mut Assets<Mesh>, root: Entity, m: &MeshData, mat: Handle<StandardMaterial>| {
        if !m.is_empty() {
            commands.spawn((Mesh3d(meshes.add(to_mesh(m))), MeshMaterial3d(mat), Transform::default(), ChildOf(root)));
        }
    };
    let add_terrain = |commands: &mut Commands, meshes: &mut Assets<Mesh>, root: Entity, m: &MeshData| {
        commands.spawn((Mesh3d(meshes.add(to_mesh(m))), MeshMaterial3d(mats.terrain.clone()), Transform::default(), ChildOf(root)));
    };

    // --- The big field under the street vignettes ---
    let root = spawn_root(commands, proj, "lookdev ground".into());
    let mut field = SurfaceMap::uniform(tile, Biome::TemperateForest);
    field.res = 96;
    field.cover = vec![Cover::Grass; 96 * 96];
    add_terrain(commands, meshes, root, &terrain::build_mesh(grid, proj, &field, 0.0));

    // --- Swatch board: 4 m cells on the field ---
    let width = tile.width_m();
    let cell_m = 4.0;
    let (su, sv) = (3000.0 / 4096.0, 3500.0 / 4096.0);
    let cell_uv = cell_m / width;
    let mut board = MeshData::default();
    let mut swatch = |col: usize, row: usize, w: [f32; LAYERS], tint: [f32; 4]| {
        let (u0, v0) = (su + col as f64 * cell_uv * 1.1, sv + row as f64 * cell_uv * 1.1);
        let n = 6;
        let base = board.positions.len() as u32;
        let up = proj.up().to_array();
        let packed = ed_world::surface::pack_splat(&w);
        for j in 0..=n {
            for i in 0..=n {
                let (u, v) = (u0 + cell_uv * i as f64 / n as f64, v0 + cell_uv * j as f64 / n as f64);
                board.vertex(proj.local(u, v, ground_h + 0.03).to_array(), up, [u as f32, v as f32], tint);
                board.splat.push(packed);
            }
        }
        for j in 0..n as u32 {
            for i in 0..n as u32 {
                let a = base + j * (n as u32 + 1) + i;
                let (b, c) = (a + 1, a + n as u32 + 1);
                board.tri(a, c, c + 1);
                board.tri(a, c + 1, b);
            }
        }
    };
    let one = |g: Ground| {
        let mut w = [0.0; LAYERS];
        w[g as usize] = 1.0;
        w
    };
    let mix = |a: Ground, b: Ground| {
        let mut w = [0.0; LAYERS];
        w[a as usize] = 0.5;
        w[b as usize] = 0.5;
        w
    };
    for (i, g) in Ground::ALL.iter().enumerate() {
        swatch(i, 0, one(*g), [1.0, 1.0, 1.0, 0.0]);
    }
    let biomes = [
        Biome::TropicalRainforest, Biome::Savanna, Biome::HotDesert, Biome::Mediterranean, Biome::TemperateForest,
        Biome::Grassland, Biome::Boreal, Biome::Tundra, Biome::Alpine, Biome::IceCap,
    ];
    for (i, b) in biomes.iter().enumerate() {
        let sm = SurfaceMap::uniform(tile, *b);
        let mut t = sm.tint(0.5, 0.5);
        t[3] = 0.0;
        swatch(i, 1, one(Ground::Grass), t);
    }
    let mixes = [
        mix(Ground::Grass, Ground::Dirt),
        mix(Ground::Grass, Ground::Moss),
        mix(Ground::ForestFloor, Ground::Moss),
        mix(Ground::DryGrass, Ground::Dirt),
        mix(Ground::Sand, Ground::Gravel),
        mix(Ground::Rock, Ground::Snow),
        mix(Ground::Gravel, Ground::Snow),
        mix(Ground::Paving, Ground::Grass),
        mix(Ground::Farmland, Ground::Dirt),
        one(Ground::Mud),
    ];
    for (i, w) in mixes.iter().enumerate() {
        let wet = if i == 9 { 1.0 } else { 0.0 };
        swatch(i, 2, *w, [1.0, 1.0, 1.0, wet]);
    }
    add_terrain(commands, meshes, root, &board);
    let board_c = proj.local(su + cell_uv * 6.6, sv + cell_uv * 1.6, ground_h) + anchor_v;
    let north = (proj.local(0.5, 0.0, 0.0) - proj.local(0.5, 1.0, 0.0)).normalize();
    let up = proj.up();
    shots.push(Shot { name: "ground_swatches".into(), eye: board_c + up * 22.0 - north * 15.0, target: board_c, tonemap: None });
    let row0 = proj.local(su + cell_uv * 2.2, sv + cell_uv * 0.5, ground_h) + anchor_v;
    shots.push(Shot { name: "ground_swatches_closeup".into(), eye: row0 + up * 3.0 - north * 7.0, target: row0 + north * 1.0, tonemap: None });

    // --- Places ---
    let c00 = TileId::new(15, tile.x * 2, tile.y * 2);
    let mut taken = std::collections::HashSet::new();
    for dy in 0..2 {
        for dx in 0..2 {
            taken.insert((dx, dy));
        }
    }
    let mut blocks = std::collections::HashMap::new();
    for p in PLACES {
        let (cx, cy) = p.offset();
        for dy in -1..=1 {
            for dx in -1..=1 {
                blocks.insert((cx + dx, cy + dy), (p, dx == 0 && dy == 0));
            }
        }
    }
    for ty in -3..=2i64 {
        for tx in -6..=5i64 {
            if taken.contains(&(tx, ty)) {
                continue;
            }
            let Some(t) = c00.offset(tx, ty) else { continue };
            let (place, center) = blocks.get(&(tx, ty)).copied().map_or((None, false), |(p, c)| (Some(p), c));
            let tp = TileProjector::new(t, origin.frame);
            let segs = if center { 64 } else { 32 };
            let (grid_t, surface_t) = place_terrain(t, c00, place, segs);
            let root = spawn_root(commands, &tp, format!("lookdev place {tx},{ty}"));
            // No skirts here: neighbouring lookdev tiles share exact edge heights,
            // and skirted tiles streak under the CPU renderer (see NOTES.md).
            let skirt = 0.0;
            let tm = terrain::build_mesh(&grid_t, &tp, &surface_t, skirt);
            add_terrain(commands, meshes, root, &tm);
            let Some(place) = place else { continue };
            // Trees for context (forest, park, scrub woods, meadow hedges).
            let occ = if place == Place::UrbanPark { Some(urban_extras(commands, meshes, mats, root, &tp, &grid_t, &surface_t)) } else { None };
            let spot = if center { Some(camera_spot(place, &grid_t, &tp)) } else { None };
            let (trees, _) = foliage::scatter(&tp, &grid_t, &surface_t, occ.as_ref(), if center { 6000 } else { 1500 });
            let trees: Vec<_> = trees
                .into_iter()
                .filter(|t| spot.is_none_or(|(su, sv, _)| t.pos.distance(tp.local(su, sv, grid_t.sample(su, sv) as f64)) > 4.5))
                .collect();
            add(commands, meshes, root, &props::bake_trees(&trees, tp.up()), mats.matte.clone());
            if !center {
                continue;
            }
            let (u, v, dir) = spot.unwrap();
            let src = GroundSource { proj: tp, grid: grid_t.clone(), surface: surface_t.clone(), occupancy: occ };
            commands.entity(root).insert(GroundCover(Arc::new(src)));
            // Camera rig at the spot, looking along `dir` (tile uv direction).
            let anchor = tp.anchor.as_vec3();
            let g = |u: f64, v: f64| grid_t.sample(u, v) as f64;
            let at = |du: f64, dv: f64| (u + du, v + dv);
            let m = 1.0 / t.width_m();
            let ahead = |dist: f64| at(dir.0 * dist * m, dir.1 * dist * m);
            let ground_pt = |(uu, vv): (f64, f64), lift: f64| tp.local(uu, vv, g(uu, vv) + lift) + anchor;
            let name = place.name();
            shots.push(Shot { name: format!("ground_{name}_stand"), eye: ground_pt((u, v), 1.5), target: ground_pt(ahead(9.0), 0.0), tonemap: None });
            shots.push(Shot { name: format!("ground_{name}_crouch"), eye: ground_pt((u, v), 0.4), target: ground_pt(ahead(2.4), 0.0), tonemap: None });
            shots.push(Shot {
                name: format!("ground_{name}_aerial"),
                eye: ground_pt(ahead(-160.0), 90.0),
                target: ground_pt(ahead(60.0), 0.0),
                tonemap: None,
            });
        }
    }
    shots
}

/// Height grid + land cover for a lookdev tile (z15).
fn place_terrain(t: TileId, c00: TileId, place: Option<Place>, segs: usize) -> (TerrainGrid, SurfaceMap) {
    let k = 2f64.powi(14 - t.z as i32);
    let global = |u: f64, v: f64| ((t.x as f64 + u) * k, (t.y as f64 + v) * k);
    let mut grid = TerrainGrid::flat(t, segs, 0.0);
    let n = segs + 1;
    for j in 0..n {
        for i in 0..n {
            let (u, v) = (i as f64 / segs as f64, j as f64 / segs as f64);
            let (gx, gy) = global(u, v);
            grid.heights[j * n + i] = height_at(place, c00, gx, gy);
        }
    }
    let biome = place.map_or(Biome::TemperateForest, |p| p.biome());
    let res = if segs >= 64 { 192 } else { 64 };
    let mut surface = SurfaceMap::uniform(t, biome);
    surface.res = res;
    if place == Some(Place::Alpine) {
        // Pull the snowline down so a 1.7 km mountain gets a snow cap here.
        surface.lat = 85.0;
    }
    surface.cover = (0..res * res)
        .map(|c| {
            let (u, v) = (((c % res) as f64 + 0.5) / res as f64, ((c / res) as f64 + 0.5) / res as f64);
            let (gx, gy) = global(u, v);
            let h = grid.sample(u, v);
            let e = 0.5 / res as f64;
            let dh = ((grid.sample(u + e, v) - grid.sample(u - e, v)).powi(2) + (grid.sample(u, v + e) - grid.sample(u, v - e)).powi(2)).sqrt();
            let run = (2.0 * e * t.width_m()) as f32;
            let slope = 1.0 - 1.0 / (1.0 + (dh / run).powi(2)).sqrt();
            match place {
                Some(p) => p.cover(u, v, gx, gy, h, slope),
                None => {
                    if fbm(gx * 18.0, gy * 18.0, 50, 4, 0) > 0.68 { Cover::Wood } else { Cover::Grass }
                }
            }
        })
        .collect();
    (grid, surface)
}

/// Height for any lookdev tile; blocks blend into the shared base at their
/// edges. `c00` is the main tile's first z15 child (block offsets are
/// relative to it).
fn height_at(place: Option<Place>, c00: TileId, gx: f64, gy: f64) -> f32 {
    let base = base_height(gx, gy);
    let Some(p) = place else { return base };
    let k = 2f64.powi(14 - c00.z as i32);
    let (ox, oy) = p.offset();
    let center = ((c00.x as i64 + ox) as f64 + 0.5, (c00.y as i64 + oy) as f64 + 0.5);
    // Distance from the block center in z15 tiles.
    let dx = gx / k - center.0;
    let dy = gy / k - center.1;
    let r = (dx * dx + dy * dy).sqrt() as f32;
    let window = 1.0 - smoothstep(0.9, 1.5, dx.abs().max(dy.abs()) as f32);
    match p {
        Place::Alpine => {
            // A craggy massif: cone + ridged noise.
            let cone = (1.0 - (r / 1.45).min(1.0)).powf(1.3);
            let ridge = 1.0 - (2.0 * fbm(gx * 6.0, gy * 6.0, 47, 5, 0) - 1.0).abs();
            base + window * (1750.0 * cone + 260.0 * ridge * cone.sqrt() + 60.0 * fbm(gx * 30.0, gy * 30.0, 48, 3, 0))
        }
        _ => base + window * p.relief(gx, gy),
    }
}

/// Where the camera stands in a place, and which way it looks (uv dir).
fn camera_spot(place: Place, grid: &TerrainGrid, tp: &TileProjector) -> (f64, f64, (f64, f64)) {
    match place {
        Place::Alpine => {
            // Find a gentle-ish spot near the snow edge, look uphill.
            let mut best = (0.5, 0.5, f32::MAX);
            for j in 8..56 {
                for i in 8..56 {
                    let (u, v) = (i as f64 / 64.0, j as f64 / 64.0);
                    let h = grid.sample(u, v);
                    let e = 1.0 / 64.0;
                    let s = (grid.sample(u + e, v) - grid.sample(u - e, v)).abs() + (grid.sample(u, v + e) - grid.sample(u, v - e)).abs();
                    let score = (h - 1380.0).abs() + s * 2.0 + ((u - 0.5).abs() + (v - 0.5).abs()) as f32 * 300.0;
                    if score < best.2 {
                        best = (u, v, score);
                    }
                }
            }
            let (u, v) = (best.0, best.1);
            let e = 1.0 / 128.0;
            let gu = (grid.sample(u + e, v) - grid.sample(u - e, v)) as f64;
            let gv = (grid.sample(u, v + e) - grid.sample(u, v - e)) as f64;
            let l = (gu * gu + gv * gv).sqrt().max(1e-6);
            let _ = tp;
            // Look along the slope (contour), a bit downhill, so the sunlit
            // flank and the snow edge are both in frame.
            let (cu, cv) = (-gv / l, gu / l);
            (u, v, (cu * 0.9 - gu / l * 0.45, cv * 0.9 - gv / l * 0.45))
        }
        // Look from the park toward the street (east).
        Place::UrbanPark => (0.535, 0.5, (0.9, -0.44)),
        _ => (0.5, 0.5, (0.0, -1.0)),
    }
}

/// Urban park edge: a street along the park, a footpath through it and a
/// row of houses across the street. Returns the occupancy mask for scatter.
fn urban_extras(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &WorldMaterials,
    root: Entity,
    tp: &TileProjector,
    grid: &TerrainGrid,
    surface: &SurfaceMap,
) -> Occupancy {
    let s = |k: &str| Value::String(k.into());
    let feat = |geom: GeomType, props: &[(&str, Value)], ring: Vec<[f32; 2]>, id: u64| mvt::Feature {
        id: Some(id),
        geom_type: geom,
        props: props.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
        rings: vec![ring],
    };
    let x_street = 0.555 * 4096.0;
    let roads_l = vec![
        feat(GeomType::LineString, &[("class", s("minor"))], vec![[x_street, 1200.0], [x_street, 2900.0]], 1),
        feat(GeomType::LineString, &[("class", s("path"))], vec![[1500.0, 2140.0], [1850.0, 2080.0], [2150.0, 2010.0]], 2),
    ];
    let mut b = Vec::new();
    let mut id = 100;
    let mut y = 1400.0f32;
    while y < 2700.0 {
        id += 1;
        let w = 40.0 + 30.0 * ed_world::hash01(id, 1);
        let x0 = x_street + 45.0;
        b.push(feat(GeomType::Polygon, &[], vec![[x0, y], [x0 + 70.0, y], [x0 + 70.0, y + w], [x0, y + w]], id));
        y += w + 12.0;
    }
    let vt = mvt::Tile {
        layers: vec![
            mvt::Layer { name: "transportation".into(), extent: 4096, features: roads_l },
            mvt::Layer { name: "building".into(), extent: 4096, features: b },
        ],
    };
    let region = Region::NorthAmericaSuburb;
    let r = roads::generate(&vt, tp, grid, surface, region);
    let add = |commands: &mut Commands, meshes: &mut Assets<Mesh>, m: &MeshData, mat: &Handle<StandardMaterial>| {
        if !m.is_empty() {
            commands.spawn((Mesh3d(meshes.add(to_mesh(m))), MeshMaterial3d(mat.clone()), Transform::default(), ChildOf(root)));
        }
    };
    add(commands, meshes, &r.mesh, &mats.matte);
    add(commands, meshes, &props::bake_street_props(&r.props, tp.up()), &mats.matte);
    let ctx = BuildingContext { proj: tp, grid, surface, biome: surface.biome };
    let bm = buildings::generate_in_region(&vt, &ctx, region);
    add(commands, meshes, &bm.facades, &mats.facade);
    add(commands, meshes, &bm.glass, &mats.glass);
    add(commands, meshes, &bm.storefronts, &mats.storefront);
    add(commands, meshes, &bm.roofs, &mats.matte);
    Occupancy::from_vector(&vt, 512)
}
