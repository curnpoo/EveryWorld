//! Materials for building geometry, one per `ed_world::bparts::Mat` bucket.
//! Textures are generated in code, tile seamlessly, and are mostly white /
//! grey so per-building vertex colors tint them (one brick texture serves
//! every brick color on Earth). Wall UVs are in meters; roof UVs are meters
//! along the eave and courses up the slope.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use ed_world::archkit::{RoofMat, WallMat};
use ed_world::bparts::Mat;
use std::collections::HashMap;

#[derive(Resource, Clone, Default)]
pub struct BuildingMaterials {
    pub by_mat: HashMap<Mat, Handle<StandardMaterial>>,
}

impl BuildingMaterials {
    pub fn get(&self, m: Mat) -> Handle<StandardMaterial> {
        self.by_mat.get(&m).cloned().unwrap_or_default()
    }
}

pub struct BuildingMaterialsPlugin;

impl Plugin for BuildingMaterialsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, setup);
    }
}

fn hash(x: i32, y: i32, s: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ s.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

/// Smooth value noise, tileable with period `p` cells.
fn vnoise(x: f32, y: f32, p: i32, s: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - xi as f32, y - yi as f32);
    let (u, v) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let g = |i: i32, j: i32| hash(i.rem_euclid(p), j.rem_euclid(p), s);
    let a = g(xi, yi) + (g(xi + 1, yi) - g(xi, yi)) * u;
    let b = g(xi, yi + 1) + (g(xi + 1, yi + 1) - g(xi, yi + 1)) * u;
    a + (b - a) * v
}

fn fbm(u: f32, v: f32, base: i32, s: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut f = base;
    for o in 0..4 {
        sum += amp * vnoise(u * f as f32, v * f as f32, f, s + o);
        amp *= 0.5;
        f *= 2;
    }
    sum
}

fn image(size: u32, f: impl Fn(f32, f32) -> [f32; 3]) -> Image {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (u, v) = ((x as f32 + 0.5) / size as f32, (y as f32 + 0.5) / size as f32);
            let c = f(u, v);
            for k in 0..3 {
                data.push((c[k].clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8);
            }
            data.push(255);
        }
    }
    let mut img = Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        crate::native::mesh_usage(),
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    });
    img
}

fn grey(x: f32) -> [f32; 3] {
    [x, x, x]
}

/// One meter of running-bond brick: 4 courses x 4 bricks, mortar lines.
fn brick() -> Image {
    image(256, |u, v| {
        let rows = 13.0; // ~75 mm courses
        let r = (v * rows).floor();
        let fy = (v * rows).fract();
        let shift = if r as i32 % 2 == 0 { 0.0 } else { 0.5 };
        let bx = u * 4.4 + shift;
        let fx = bx.fract();
        let mortar = fy < 0.12 || fx < 0.035;
        if mortar {
            grey(0.78 + 0.05 * fbm(u, v, 16, 1))
        } else {
            let id = hash(bx.floor() as i32, r as i32, 7);
            let n = fbm(u, v, 32, 3);
            grey(0.62 + 0.28 * id + 0.12 * n)
        }
    })
}

/// Lime plaster / stucco: soft mottling and fine grain.
fn plaster() -> Image {
    image(256, |u, v| grey(0.86 + 0.1 * fbm(u, v, 4, 11) + 0.04 * fbm(u, v, 64, 12)))
}

/// Dressed stone blocks (2 m tile): ashlar courses with fine joints.
fn stone() -> Image {
    image(256, |u, v| {
        let rows = 4.0;
        let r = (v * rows).floor();
        let fy = (v * rows).fract();
        let shift = if r as i32 % 2 == 0 { 0.0 } else { 0.37 };
        let bx = u * 2.0 + shift;
        let joint = fy < 0.025 || bx.fract() < 0.012;
        let id = hash(bx.floor() as i32, r as i32, 21);
        let n = fbm(u, v, 16, 22);
        if joint { grey(0.62) } else { grey(0.8 + 0.08 * id + 0.1 * n) }
    })
}

/// Clapboard siding (1.2 m tile): horizontal boards with shadow lines.
fn siding() -> Image {
    image(256, |u, v| {
        let boards = 7.0;
        let f = (v * boards).fract();
        let shade = if f < 0.12 { 0.55 + f * 2.5 } else { 0.92 - 0.06 * f };
        grey(shade + 0.04 * fbm(u, v * 0.2, 8, 31))
    })
}

/// Concrete panels (2 m tile): panel joints and stains.
fn concrete() -> Image {
    image(256, |u, v| {
        let joint = (u * 2.0).fract() < 0.01 || (v * 2.0).fract() < 0.01;
        let stain = fbm(u, v, 4, 41);
        let streak = fbm(u * 6.0, v * 0.5, 8, 42) * (1.0 - v);
        if joint { grey(0.6) } else { grey(0.82 + 0.08 * stain - 0.08 * streak) }
    })
}

/// Vertical timber boards (1.2 m tile) with grain.
fn timber() -> Image {
    image(256, |u, v| {
        let boards = 6.0;
        let f = (u * boards).fract();
        let id = hash((u * boards).floor() as i32, 0, 51);
        let grain = (v * 40.0 + fbm(u, v, 8, 52) * 6.0).sin() * 0.5 + 0.5;
        let edge = if f < 0.05 { 0.55 } else { 1.0 };
        grey((0.7 + 0.15 * id + 0.12 * grain) * edge)
    })
}

/// Clay tiles: one course per v unit, 4 tiles per meter along the eave.
fn clay_tile() -> Image {
    image(256, |u, v| {
        let x = (u * 4.0).fract();
        let id = hash((u * 4.0).floor() as i32, 0, 61);
        // Barrel profile: bright crown, dark troughs.
        let crown = (x * std::f32::consts::PI).sin().powf(0.7);
        let lip = if v < 0.12 { 0.75 } else { 1.0 };
        grey((0.55 + 0.4 * crown) * lip * (0.85 + 0.2 * id))
    })
}

/// Slate / shingle: staggered rectangles per course.
fn slate() -> Image {
    image(256, |u, v| {
        let x = u * 3.3 + if v > 0.5 { 0.5 } else { 0.0 };
        let gap = x.fract() < 0.03;
        let id = hash(x.floor() as i32, (v * 2.0) as i32, 71);
        let edge = if v < 0.08 { 0.7 } else { 1.0 };
        if gap { grey(0.35) } else { grey((0.75 + 0.2 * id + 0.06 * fbm(u, v, 16, 72)) * edge) }
    })
}

/// Standing-seam zinc / corrugated metal: seams along the slope.
fn metal_seams() -> Image {
    image(256, |u, v| {
        let x = (u * 2.0).fract();
        let seam = (x - 0.5).abs() < 0.02;
        grey(if seam { 1.0 } else { 0.82 + 0.06 * fbm(u, v, 8, 81) })
    })
}

fn gravel() -> Image {
    image(256, |u, v| grey(0.75 + 0.18 * hash((u * 256.0) as i32, (v * 256.0) as i32, 91) * fbm(u, v, 32, 92) + 0.05 * fbm(u, v, 4, 93)))
}

fn paving() -> Image {
    image(256, |u, v| {
        let joint = (u * 2.0).fract() < 0.015 || (v * 2.0).fract() < 0.015;
        grey(if joint { 0.7 } else { 0.86 + 0.08 * fbm(u, v, 8, 101) })
    })
}

/// Distant facade cell: one bay x one floor, window centered, wall around it.
fn lod_facade() -> Image {
    image(128, |u, v| {
        let v = 1.0 - v;
        let (wx0, wx1, wy0, wy1) = (0.28, 0.72, 0.28, 0.84);
        if u > wx0 && u < wx1 && v > wy0 && v < wy1 {
            let frame = u < wx0 + 0.04 || u > wx1 - 0.04 || v < wy0 + 0.04 || v > wy1 - 0.04 || (u - 0.5).abs() < 0.015;
            if frame { grey(0.95) } else {
                let t = (v - wy0) / (wy1 - wy0);
                [0.12 + 0.1 * t, 0.16 + 0.12 * t, 0.22 + 0.16 * t]
            }
        } else if v > wy0 - 0.06 && v < wy0 && u > wx0 - 0.04 && u < wx1 + 0.04 {
            grey(0.8) // sill
        } else {
            grey(0.92 + 0.05 * fbm(u, v, 4, 111))
        }
    })
}

fn setup(mut commands: Commands, mut mats: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    let mut by_mat = HashMap::new();
    let tex = |img: Image, rough: f32, reflect: f32, metal: f32, mats: &mut Assets<StandardMaterial>, images: &mut Assets<Image>| {
        mats.add(StandardMaterial {
            base_color_texture: Some(images.add(img)),
            perceptual_roughness: rough,
            reflectance: reflect,
            metallic: metal,
            ..default()
        })
    };
    by_mat.insert(Mat::Wall(WallMat::Brick), tex(brick(), 0.85, 0.3, 0.0, &mut mats, &mut images));
    by_mat.insert(Mat::Wall(WallMat::Plaster), tex(plaster(), 0.9, 0.3, 0.0, &mut mats, &mut images));
    by_mat.insert(Mat::Wall(WallMat::Stone), tex(stone(), 0.8, 0.3, 0.0, &mut mats, &mut images));
    by_mat.insert(Mat::Wall(WallMat::Siding), tex(siding(), 0.7, 0.35, 0.0, &mut mats, &mut images));
    by_mat.insert(Mat::Wall(WallMat::Concrete), tex(concrete(), 0.85, 0.3, 0.0, &mut mats, &mut images));
    by_mat.insert(Mat::Wall(WallMat::Timber), tex(timber(), 0.8, 0.3, 0.0, &mut mats, &mut images));
    by_mat.insert(Mat::Roof(RoofMat::ClayTile), tex(clay_tile(), 0.75, 0.3, 0.0, &mut mats, &mut images));
    by_mat.insert(Mat::Roof(RoofMat::Slate), tex(slate(), 0.55, 0.4, 0.0, &mut mats, &mut images));
    by_mat.insert(Mat::Roof(RoofMat::Shingle), tex(slate(), 0.85, 0.3, 0.0, &mut mats, &mut images));
    by_mat.insert(Mat::Roof(RoofMat::Zinc), tex(metal_seams(), 0.45, 0.5, 0.6, &mut mats, &mut images));
    by_mat.insert(Mat::Roof(RoofMat::Metal), tex(metal_seams(), 0.5, 0.5, 0.5, &mut mats, &mut images));
    by_mat.insert(Mat::Roof(RoofMat::Gravel), tex(gravel(), 0.95, 0.25, 0.0, &mut mats, &mut images));
    by_mat.insert(Mat::Paving, tex(paving(), 0.9, 0.3, 0.0, &mut mats, &mut images));
    by_mat.insert(Mat::LodWall, tex(lod_facade(), 0.8, 0.35, 0.0, &mut mats, &mut images));
    by_mat.insert(Mat::Glass, mats.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.06, reflectance: 0.9, metallic: 0.1, ..default() }));
    by_mat.insert(Mat::Trim, mats.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.6, reflectance: 0.35, ..default() }));
    by_mat.insert(Mat::Metal, mats.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.45, metallic: 0.6, ..default() }));
    by_mat.insert(Mat::Fabric, mats.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, reflectance: 0.2, ..default() }));
    by_mat.insert(Mat::Plant, mats.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.9, reflectance: 0.25, ..default() }));
    commands.insert_resource(BuildingMaterials { by_mat });
}
