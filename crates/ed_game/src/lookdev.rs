//! Lookdev: art-direction review boards. `--lookdev <dir>` builds a stage
//! with every region's street vignette, a tree lineup, ground-cover swatches
//! and cloud styles, then flies the camera through a shot list, saving
//! `<dir>/<shot>.png` for each. Same renderer, materials and generators as
//! the real game, so what we approve is what ships.

use crate::assets::WorldMaterials;
use crate::meshconv::to_mesh;
use crate::origin::{GeoAnchored, WorldOrigin};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use ed_data::mvt::{self, GeomType, Value};
use ed_geo::tile::TileId;
use ed_world::biome::Biome;
use ed_world::buildings::{self, BuildingContext};
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
const SPACING_PX: f32 = 700.0;

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
            if matches!(region, Region::NorthAmericaCity | Region::EastAsia) && ed_world::hash01(id, 3) < 0.25 {
                props.push(("render_height", Value::Double(st.floor_height as f64 * (8.0 + 20.0 * ed_world::hash01(id, 4) as f64))));
            }
            b.push(feat(GeomType::Polygon, &props, vec![[x, ya], [x + w, ya], [x + w, yb], [x, yb]], id));
            let attached = matches!(
                region,
                Region::NorthAmericaCity | Region::Paris | Region::British | Region::WesternEurope | Region::LatinAmerica
                    | Region::NorthAfricaMiddleEast | Region::SoutheastAsia | Region::Mediterranean | Region::EasternEurope | Region::SouthAsia
            );
            x += w + if attached { 0.0 } else { 6.0 + 10.0 * ed_world::hash01(id, 5) };
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
    tonemap: Option<Tonemapping>,
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
    bmats: Res<crate::building_materials::BuildingMaterials>,
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
    let bmats = bmats.clone();
    let anchor_v = proj.anchor.as_vec3();
    // Mesh space (relative to the tile anchor) and world space (for cameras).
    let to_local = |px: f32, py: f32, h: f64| proj.local(px as f64 / ext as f64, py as f64 / ext as f64, h);
    let to_world = |px: f32, py: f32, h: f64| to_local(px, py, h) + anchor_v;
    let mut shots = Vec::new();

    // --- Ground: one big flat grass field, with swatch grid in one corner ---
    let grid = TerrainGrid::flat(tile, 96, GROUND_H as f32);
    let mut surface = SurfaceMap::uniform(tile, Biome::TemperateForest);
    let res = 96usize;
    surface.res = res;
    surface.cover = vec![Cover::Grass; res * res];
    let mut ground = terrain::build_mesh(&grid, &proj, &surface, 0.0);
    add(&ground, &mats.terrain);
    ground = MeshData::default();
    // Crisp swatch tiles: row 0 = biome default ground, row 1 = mapped land
    // covers, row 2 = the slope/snow rules (flat, steep, high, high+steep).
    let biomes = [
        Biome::TropicalRainforest, Biome::Savanna, Biome::HotDesert, Biome::Mediterranean, Biome::TemperateForest,
        Biome::Grassland, Biome::Boreal, Biome::Tundra, Biome::Alpine, Biome::IceCap,
    ];
    let covers = [
        Cover::Wood, Cover::Grass, Cover::Farmland, Cover::Wetland, Cover::Sand, Cover::Rock, Cover::Ice,
        Cover::Residential, Cover::Commercial, Cover::Industrial, Cover::Park, Cover::Pitch,
    ];
    let (sx, sy, cell) = (3000.0f32, 3500.0f32, 60.0f32); // px; 60 px ~ 27 m
    let mut swatch = |col: usize, row: usize, color: [f32; 4]| {
        let x0 = sx + col as f32 * cell;
        let y0 = sy + row as f32 * cell;
        let h = GROUND_H + 0.08;
        let q = [to_local(x0, y0 + cell * 0.88, h), to_local(x0 + cell * 0.88, y0 + cell * 0.88, h), to_local(x0 + cell * 0.88, y0, h), to_local(x0, y0, h)];
        ground.quad(q, [[0.0; 2]; 4], color);
    };
    for (i, b) in biomes.iter().enumerate() {
        let sm = SurfaceMap::uniform(tile, *b);
        swatch(i, 0, sm.ground_color(0.5, 0.5, 200.0, 0.0));
    }
    for (i, c) in covers.iter().enumerate() {
        let mut sm = SurfaceMap::uniform(tile, Biome::TemperateForest);
        sm.cover = vec![*c];
        swatch(i, 1, sm.ground_color(0.5, 0.5, 200.0, 0.0));
    }
    let alpine = SurfaceMap::uniform(tile, Biome::Alpine);
    let snowline = Biome::snowline_m(alpine.lat);
    for (i, (h, slope)) in [(1_500.0, 0.0), (1_500.0, 0.6), (snowline - 200.0, 0.1), (snowline + 300.0, 0.1), (snowline + 300.0, 0.75), (1.0, 0.0)].iter().enumerate() {
        swatch(i, 2, alpine.ground_color(0.5, 0.5, *h, *slope));
    }
    add(&ground, &mats.terrain);
    let sw_center = to_world(sx + cell * 6.0, sy + cell * 1.5, GROUND_H);
    shots.push(Shot { name: "ground_swatches".into(), eye: sw_center + up * 150.0 + (to_local(0.0, 150.0, 0.0) - to_local(0.0, 0.0, 0.0)), target: sw_center, tonemap: None });

    // --- Region street vignettes ---
    let regions: Vec<Region> = Region::ALL.to_vec();
    for (i, region) in regions.iter().enumerate() {
        let (cx, cy) = (500.0 + (i % 4) as f32 * SPACING_PX, 500.0 + (i / 4) as f32 * SPACING_PX);
        let vt = vignette_tile(cx, cy, *region, i as u64);
        let biome = region_biome(*region);
        let mut sm = SurfaceMap::uniform(tile, biome);
        sm.cover = vec![Cover::Residential];
        let r = roads::generate(&vt, &proj, &grid, &sm, *region);
        add(&r.mesh, &mats.matte);
        add(&props::bake_street_props(&r.props, up), &mats.matte);
        let ctx = BuildingContext { proj: &proj, grid: &grid, surface: &sm, biome, roads: &r.centerlines };
        let bm = buildings::generate_in_region(&vt, &ctx, *region);
        for (mat, mesh) in &bm.out.meshes {
            add(mesh, &bmats.get(*mat));
        }
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
        let c = to_world(cx, cy, GROUND_H);
        let name = format!("region_{}", format!("{region:?}").to_lowercase());
        // Three-quarter street view from the side street, a bit above head height.
        shots.push(Shot { name: format!("{name}_street"), eye: to_world(cx + 60.0, cy + 240.0, GROUND_H + 9.0), target: c + up * 6.0, tonemap: None });
        // Eye level on the main street, looking down the north-side facades.
        shots.push(Shot { name: format!("{name}_closeup"), eye: to_world(cx - 150.0, cy + 6.0, GROUND_H + 1.7), target: to_world(cx - 60.0, cy - 34.0, GROUND_H + 7.0), tonemap: None });
        if *region == Region::Mediterranean {
            for (tag, tm) in [("agx", Tonemapping::AgX), ("tony", Tonemapping::TonyMcMapface), ("aces", Tonemapping::AcesFitted), ("blender", Tonemapping::BlenderFilmic)] {
                shots.push(Shot { name: format!("tonemap_{tag}"), eye: to_world(cx + 60.0, cy + 240.0, GROUND_H + 9.0), target: c + up * 6.0, tonemap: Some(tm) });
            }
        }
        shots.push(Shot { name: format!("{name}_aerial"), eye: to_world(cx - 180.0, cy + 300.0, GROUND_H + 110.0), target: c, tonemap: None });
    }

    // --- Tree lineup ---
    {
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
        let mid = to_world(lx + 3.5 * 26.0, ly, GROUND_H);
        shots.push(Shot { name: "trees_lineup".into(), eye: mid + up * 9.0 + to_local(0.0, 120.0, 0.0) - to_local(0.0, 0.0, 0.0), target: mid + up * 5.0, tonemap: None });

    }

    // --- Clouds (volumetric sky, see clouds.rs for each shot's weather) ---
    shots.extend(cloud_shots(to_world(3400.0, 2000.0, GROUND_H)));

    // --- Robot courier ---
    {
        let spot = to_world(3400.0, 1500.0, GROUND_H);
        let up_w = origin.up_at(spot);
        let pos = spot + up_w * crate::player::RIDE_HEIGHT;
        commands.spawn((
            crate::player::Player::idle(0.5, crate::player::MoveMode::Walk),
            avian3d::prelude::LinearVelocity::default(),
            Transform::from_translation(pos),
        ));
        let fwd = Quat::from_rotation_arc(Vec3::Y, up_w) * Quat::from_rotation_y(0.5) * Vec3::NEG_Z;
        let right = fwd.cross(up_w);
        shots.push(Shot { name: "character_front".into(), eye: pos + fwd * 2.3 + right * 0.9 + up_w * 0.3, target: pos + up_w * 0.05, tonemap: None });
        shots.push(Shot { name: "character_back".into(), eye: pos - fwd * 2.2 - right * 1.1 + up_w * 0.7, target: pos + up_w * 0.05, tonemap: None });
    }

    // `--only a,b`: keep shots whose name contains any of the terms.
    if let Some(only) = &ld.only {
        let terms: Vec<String> = only.split(',').map(|t| t.trim().to_lowercase()).collect();
        shots.retain(|s| terms.iter().any(|t| s.name.contains(t.as_str())));
    }
    commands.insert_resource(ShotList { shots, idx: 0, frames: 0 });
    let _ = HashMap::<u8, u8>::new();
}

/// The shot currently on camera (cloud presets key off its name).
#[derive(Resource)]
pub struct ActiveShot(pub String);

/// Volumetric cloud review shots around a stage point. Weather, time of day
/// and storm placement for each come from `clouds::apply_lookdev_presets`.
fn cloud_shots(stage: Vec3) -> Vec<Shot> {
    crate::clouds::LOOKDEV_SHOTS
        .iter()
        .map(|s| {
            let eye = stage + Vec3::Y * s.height_m;
            let dir = crate::clouds::heading_pitch_dir(s.heading_deg, s.pitch_deg);
            Shot { name: s.name.into(), eye, target: eye + dir * 1_000.0, tonemap: None }
        })
        .collect()
}

fn run_shots(
    mut commands: Commands,
    ld: Res<Lookdev>,
    list: Option<ResMut<ShotList>>,
    origin: Res<WorldOrigin>,
    mut cam: Query<(&mut Transform, &mut Tonemapping), With<Camera3d>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut list) = list else { return };
    let Ok((mut t, mut tm)) = cam.single_mut() else { return };
    if list.idx >= list.shots.len() {
        list.frames += 1;
        if list.frames > 15 {
            exit.write(AppExit::Success);
        }
        return;
    }
    let (eye, target, name, want_tm) = {
        let shot = &list.shots[list.idx];
        (shot.eye, shot.target, shot.name.clone(), shot.tonemap.unwrap_or(Tonemapping::BlenderFilmic))
    };
    if list.frames == 0 {
        commands.insert_resource(ActiveShot(name.clone()));
    }
    if *tm != want_tm {
        *tm = want_tm;
    }
    let up = origin.up_at(eye);
    *t = Transform::from_translation(eye).looking_at(target, up);
    list.frames += 1;
    // Let TAA/auto-exposure/env-map settle on each new view.
    // First shot waits for shader compilation and the sky LUTs.
    let settle = if list.idx == 0 { 60 } else { 12 };
    if list.frames == settle {
        let path = ld.out_dir.join(format!("{name}.png"));
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if list.frames > settle + 2 {
        list.idx += 1;
        list.frames = 0;
    }
}
