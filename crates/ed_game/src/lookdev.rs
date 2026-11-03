//! Lookdev: art-direction review boards. `--lookdev <dir>` builds a stage
//! with every region's street vignette, a tree lineup, ground-cover swatches
//! and cloud styles, then flies the camera through a shot list, saving
//! `<dir>/<shot>.png` for each. Same renderer, materials and generators as
//! the real game, so what we approve is what ships.

use crate::assets::WorldMaterials;
use crate::meshconv::to_mesh;
use crate::origin::{GeoAnchored, WorldOrigin};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use ed_data::mvt::{self, GeomType, Value};
use ed_geo::tile::TileId;
use ed_world::biome::Biome;
use ed_world::buildings::{self, BuildingContext};
use ed_world::clouds::{self, CloudStyle};
use ed_world::foliage::{self, TreeInstance, TreeKind};
use ed_world::mesh::MeshData;
use ed_world::project::TileProjector;
use ed_world::style::Region;
use ed_world::surface::{Cover, SurfaceMap};
use ed_world::terrain::{self, TerrainGrid};
use ed_world::{props, roads};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct LookdevPlugin;

impl Plugin for LookdevPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, build_stage).add_systems(Update, run_shots);
    }
}

#[derive(Resource)]
pub struct Lookdev {
    pub out_dir: PathBuf,
    pub only: Option<String>,
}

const GROUND_H: f64 = 50.0;
const SPACING_PX: f32 = 780.0;

fn region_biome(r: Region) -> Biome {
    match r {
        Region::Mediterranean | Region::Oceania => Biome::Mediterranean,
        Region::Alpine => Biome::Alpine,
        Region::Nordic => Biome::Boreal,
        Region::NorthAfricaMiddleEast => Biome::HotDesert,
        Region::SubSaharan | Region::SouthAsia => Biome::Savanna,
        Region::SoutheastAsia | Region::LatinAmerica => Biome::TropicalRainforest,
        _ => Biome::TemperateForest,
    }
}

fn feat(geom: GeomType, props: &[(&str, Value)], ring: Vec<[f32; 2]>, id: u64) -> mvt::Feature {
    mvt::Feature {
        id: Some(id),
        geom_type: geom,
        props: props.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
        rings: vec![ring],
    }
}

/// A synthetic street: T-junction with building rows on both sides.
fn vignette_tile(cx: f32, cy: f32, region: Region, seed: u64) -> mvt::Tile {
    let s = |k: &str| Value::String(k.into());
    let mut roads_l = vec![
        feat(GeomType::LineString, &[("class", s("secondary"))], vec![[cx - 340.0, cy], [cx, cy]], 1),
        feat(GeomType::LineString, &[("class", s("secondary"))], vec![[cx, cy], [cx + 340.0, cy]], 2),
        feat(GeomType::LineString, &[("class", s("minor"))], vec![[cx, cy], [cx, cy + 300.0]], 3),
    ];
    roads_l.push(feat(GeomType::LineString, &[("class", s("path"))], vec![[cx - 300.0, cy - 120.0], [cx + 300.0, cy - 120.0]], 4));
    let mut b = Vec::new();
    let st = region.style();
    let mut id = 100 + seed * 1000;
    for side in [-1.0f32, 1.0] {
        let mut x = cx - 320.0;
        while x < cx + 300.0 {
            id += 1;
            let r = ed_world::hash01(id, 1);
            let w = 18.0 + 22.0 * r;
            let d = 26.0 + 20.0 * ed_world::hash01(id, 2);
            // Leave the side street open.
            if side > 0.0 && (x + w > cx - 14.0) && (x < cx + 14.0) {
                x = cx + 14.0;
                continue;
            }
            let y0 = cy + side * 22.0;
            let y1 = y0 + side * d;
            let (ya, yb) = if side > 0.0 { (y0, y1) } else { (y1, y0) };
            let mut props: Vec<(&str, Value)> = Vec::new();
            // A few taller landmarks in dense regions.
            if matches!(region, Region::NorthAmericaCity | Region::EastAsia | Region::Paris) && ed_world::hash01(id, 3) < 0.25 {
                props.push(("render_height", Value::Double(st.floor_height as f64 * (8.0 + 20.0 * ed_world::hash01(id, 4) as f64))));
            }
            b.push(feat(GeomType::Polygon, &props, vec![[x, ya], [x + w, ya], [x + w, yb], [x, yb]], id));
            x += w + 1.5 + 6.0 * ed_world::hash01(id, 5);
        }
    }
    mvt::Tile {
        layers: vec![
            mvt::Layer { name: "transportation".into(), extent: 4096, features: roads_l },
            mvt::Layer { name: "building".into(), extent: 4096, features: b },
        ],
    }
}

struct Shot {
    name: String,
    eye: Vec3,
    target: Vec3,
}

#[derive(Resource)]
struct ShotList {
    shots: Vec<Shot>,
    idx: usize,
    frames: u32,
}

#[allow(clippy::too_many_arguments)]
fn build_stage(
    mut commands: Commands,
    ld: Res<Lookdev>,
    origin: Res<WorldOrigin>,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<WorldMaterials>,
) {
    let tile = TileId::containing(origin.frame.origin.lat_deg, origin.frame.origin.lon_deg, 14);
    let proj = TileProjector::new(tile, origin.frame);
    let up = proj.up();
    let ext = 4096.0f32;
    let anchor = GeoAnchored::from_render(&origin.frame, proj.anchor);
    let root = commands.spawn((anchor, Transform::default(), Visibility::default(), Name::new("lookdev"))).id();
    let mut add = |m: &MeshData, mat: &Handle<StandardMaterial>| {
        if !m.is_empty() {
            commands.spawn((Mesh3d(meshes.add(to_mesh(m))), MeshMaterial3d(mat.clone()), Transform::default(), ChildOf(root)));
        }
    };
    let to_local = |px: f32, py: f32, h: f64| proj.local(px as f64 / ext as f64, py as f64 / ext as f64, h);
    let mut shots = Vec::new();

    // --- Ground: one big flat grass field, with swatch grid in one corner ---
    let grid = TerrainGrid::flat(tile, 96, GROUND_H as f32);
    let mut surface = SurfaceMap::uniform(tile, Biome::TemperateForest);
    let res = 96usize;
    surface.res = res;
    surface.cover = vec![Cover::Grass; res * res];
    // Swatches: rows = biomes (Natural cover), plus a row of each cover type.
    let biomes = [
        Biome::TropicalRainforest, Biome::Savanna, Biome::HotDesert, Biome::Mediterranean, Biome::TemperateForest,
        Biome::Grassland, Biome::Boreal, Biome::Tundra, Biome::Alpine, Biome::IceCap,
    ];
    let covers = [
        Cover::Wood, Cover::Grass, Cover::Farmland, Cover::Wetland, Cover::Sand, Cover::Rock, Cover::Ice,
        Cover::Residential, Cover::Commercial, Cover::Industrial, Cover::Park, Cover::Pitch,
    ];
    // Bottom-right corner of the tile: cells (72..96, 72..96).
    for (i, c) in covers.iter().enumerate() {
        let (x, y) = (72 + (i % 6) * 4, 72 + (i / 6) * 4);
        for dy in 0..3 {
            for dx in 0..3 {
                surface.cover[(y + dy) * res + x + dx] = *c;
            }
        }
    }
    let mut ground = terrain::build_mesh(&grid, &proj, &surface, 0.0);
    // Biome swatches need their own SurfaceMap biome; build small patches.
    for (i, b) in biomes.iter().enumerate() {
        let mut g = TerrainGrid::flat(tile, 4, (GROUND_H + 0.05) as f32);
        let sm = SurfaceMap::uniform(tile, *b);
        g.segments = 4;
        let m = terrain::build_mesh(&g, &proj, &sm, 0.0);
        // Shrink the whole-tile patch down to a 40 m square at the swatch spot.
        let center = to_local(3000.0 + (i % 5) as f32 * 110.0, 3700.0 + (i / 5) as f32 * 110.0, GROUND_H + 0.05);
        let mut small = m.clone();
        let tile_w = tile.width_m() as f32;
        for p in &mut small.positions {
            let v = Vec3::from(*p);
            let flat = v.reject_from_normalized(up) * (40.0 / tile_w);
            *p = (center + flat + up * 0.0).to_array();
        }
        ground.append(&small);
    }
    add(&ground, &mats.terrain);
    let sw_center = to_local(3200.0, 3720.0, GROUND_H);
    shots.push(Shot { name: "ground_biomes".into(), eye: sw_center + up * 160.0 + Vec3::new(0.0, 0.0, 170.0), target: sw_center });
    let cov_center = to_local(72.0 / 96.0 * ext + 260.0, 72.0 / 96.0 * ext + 120.0, GROUND_H);
    shots.push(Shot { name: "ground_covers".into(), eye: cov_center + up * 170.0 + Vec3::new(0.0, 0.0, 160.0), target: cov_center });

    // --- Region street vignettes ---
    let mut regions: Vec<Region> = Region::ALL.to_vec();
    if let Some(only) = &ld.only {
        regions.retain(|r| format!("{r:?}").to_lowercase().contains(&only.to_lowercase()));
    }
    for (i, region) in regions.iter().enumerate() {
        let (cx, cy) = (500.0 + (i % 4) as f32 * SPACING_PX, 500.0 + (i / 4) as f32 * SPACING_PX);
        let vt = vignette_tile(cx, cy, *region, i as u64);
        let biome = region_biome(*region);
        let mut sm = SurfaceMap::uniform(tile, biome);
        sm.cover = vec![Cover::Residential];
        let r = roads::generate(&vt, &proj, &grid, &sm, *region);
        add(&r.mesh, &mats.matte);
        add(&props::bake_street_props(&r.props, up), &mats.matte);
        let ctx = BuildingContext { proj: &proj, grid: &grid, surface: &sm, biome };
        let bm = buildings::generate_in_region(&vt, &ctx, *region);
        add(&bm.facades, &mats.facade);
        add(&bm.glass, &mats.glass);
        add(&bm.storefronts, &mats.storefront);
        add(&bm.roofs, &mats.matte);
        // Street trees between the path and the buildings.
        let trees: Vec<TreeInstance> = (0..7)
            .map(|k| {
                let px = cx - 280.0 + k as f32 * 90.0;
                let rr = ed_world::hash01(i as u64, k);
                let kind = match biome {
                    Biome::Mediterranean => [TreeKind::Cypress, TreeKind::Palm, TreeKind::Broadleaf][k as usize % 3],
                    Biome::HotDesert => [TreeKind::Palm, TreeKind::Shrub][k as usize % 2],
                    Biome::Savanna => [TreeKind::Acacia, TreeKind::Shrub][k as usize % 2],
                    Biome::TropicalRainforest => [TreeKind::Palm, TreeKind::Broadleaf][k as usize % 2],
                    Biome::Boreal | Biome::Alpine => [TreeKind::Conifer, TreeKind::Birch][k as usize % 2],
                    _ => [TreeKind::Broadleaf, TreeKind::Broadleaf, TreeKind::Birch][k as usize % 3],
                };
                TreeInstance {
                    kind,
                    pos: to_local(px, cy - 150.0, GROUND_H),
                    scale: 0.8 + 0.3 * rr,
                    yaw: rr * 9.0,
                    tint: foliage::foliage_color(kind, biome, rr),
                }
            })
            .collect();
        add(&props::bake_trees(&trees, up), &mats.matte);
        let c = to_local(cx, cy, GROUND_H);
        let name = format!("region_{}", format!("{region:?}").to_lowercase());
        // Three-quarter street view from the side street, a bit above head height.
        shots.push(Shot { name: format!("{name}_street"), eye: to_local(cx + 60.0, cy + 240.0, GROUND_H + 9.0), target: c + up * 6.0 });
        shots.push(Shot { name: format!("{name}_aerial"), eye: to_local(cx - 180.0, cy + 300.0, GROUND_H + 110.0), target: c });
    }

    // --- Tree lineup ---
    if ld.only.is_none() {
        let kinds = [
            TreeKind::Broadleaf, TreeKind::Conifer, TreeKind::Birch, TreeKind::Palm,
            TreeKind::Acacia, TreeKind::Cypress, TreeKind::Shrub, TreeKind::Cactus,
        ];
        let biome_for = |k: TreeKind| match k {
            TreeKind::Conifer | TreeKind::Birch => Biome::Boreal,
            TreeKind::Palm => Biome::TropicalRainforest,
            TreeKind::Acacia => Biome::Savanna,
            TreeKind::Cypress => Biome::Mediterranean,
            TreeKind::Cactus => Biome::HotDesert,
            _ => Biome::TemperateForest,
        };
        let (lx, ly) = (3400.0, 900.0);
        let trees: Vec<TreeInstance> = kinds
            .iter()
            .enumerate()
            .map(|(i, k)| TreeInstance {
                kind: *k,
                pos: to_local(lx + i as f32 * 26.0, ly, GROUND_H),
                scale: 1.0,
                yaw: 0.6,
                tint: foliage::foliage_color(*k, biome_for(*k), 0.5),
            })
            .collect();
        add(&props::bake_trees(&trees, up), &mats.matte);
        let mid = to_local(lx + 3.5 * 26.0, ly, GROUND_H);
        shots.push(Shot { name: "trees_lineup".into(), eye: mid + up * 9.0 + to_local(0.0, 120.0, 0.0) - to_local(0.0, 0.0, 0.0), target: mid + up * 5.0 });

        // --- Clouds ---
        let (cx, cy) = (3400.0, 2000.0);
        let mut sky = MeshData::default();
        let mut place_cloud = |m: MeshData, at: Vec3| {
            let mut c = m;
            for p in &mut c.positions {
                *p = (Vec3::from(*p) + at).to_array();
            }
            sky.append(&c);
        };
        for (k, style) in [CloudStyle::Puffy, CloudStyle::Towering, CloudStyle::Stratus].iter().enumerate() {
            let size = match style { CloudStyle::Towering => 160.0, CloudStyle::Stratus => 260.0, CloudStyle::Puffy => 180.0 };
            place_cloud(clouds::cloud(*style, 7 + k as u64, size), to_local(cx + (k as f32 - 1.0) * 600.0, cy, GROUND_H + 300.0));
        }
        let cloud_mat = commands_cloud_material(&mats);
        add(&sky, &cloud_mat);
        let cc = to_local(cx, cy, GROUND_H + 330.0);
        shots.push(Shot { name: "clouds".into(), eye: to_local(cx, cy + 1400.0, GROUND_H + 120.0), target: cc });
    }

    commands.insert_resource(ShotList { shots, idx: 0, frames: 0 });
    let _ = HashMap::<u8, u8>::new();
}

fn commands_cloud_material(mats: &WorldMaterials) -> Handle<StandardMaterial> {
    mats.cloud.clone()
}

fn run_shots(
    mut commands: Commands,
    ld: Res<Lookdev>,
    list: Option<ResMut<ShotList>>,
    origin: Res<WorldOrigin>,
    mut cam: Query<&mut Transform, With<Camera3d>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut list) = list else { return };
    let Ok(mut t) = cam.single_mut() else { return };
    if list.idx >= list.shots.len() {
        list.frames += 1;
        if list.frames > 15 {
            exit.write(AppExit::Success);
        }
        return;
    }
    let shot = &list.shots[list.idx];
    let up = origin.up_at(shot.eye);
    *t = Transform::from_translation(shot.eye).looking_at(shot.target, up);
    list.frames += 1;
    // Let TAA/auto-exposure/env-map settle on each new view.
    if list.frames == 45 {
        let path = ld.out_dir.join(format!("{}.png", shot.name));
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if list.frames > 50 {
        list.idx += 1;
        list.frames = 0;
    }
}
