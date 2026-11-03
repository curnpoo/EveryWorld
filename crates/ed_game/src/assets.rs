//! Shared materials and procedural textures. Every material in the world
//! is defined here so art direction lives in one place.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        crate::ground_material::register(app);
        app.add_plugins(crate::ground_scatter::GroundScatterPlugin).add_systems(PreStartup, setup_materials);
    }
}

#[derive(Resource, Clone)]
pub struct WorldMaterials {
    /// Ground v2 splat material (see `ground_material`).
    pub terrain: Handle<crate::ground_material::TerrainMaterial>,
    pub facade: Handle<StandardMaterial>,
    pub glass: Handle<StandardMaterial>,
    pub storefront: Handle<StandardMaterial>,
    /// Untextured, vertex-colored, matte (roofs, roads, trees, props, rocks).
    pub matte: Handle<StandardMaterial>,
    pub water: Handle<StandardMaterial>,
    pub ocean: Handle<StandardMaterial>,
}

fn repeat_sampler() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    })
}

/// Build an RGBA8 sRGB image from a per-pixel function.
fn procedural(size: u32, f: impl Fn(f32, f32) -> [u8; 4]) -> Image {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (u, v) = ((x as f32 + 0.5) / size as f32, (y as f32 + 0.5) / size as f32);
            data.extend_from_slice(&f(u, v));
        }
    }
    let mut img = Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = repeat_sampler();
    img
}

fn in_rect(u: f32, v: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
    u >= x0 && u <= x1 && v >= y0 && v <= y1
}

/// One window bay x one floor. White wall (vertex color tints it), framed
/// window with a sill. v=0 is the *top* of the image; facade UV v grows
/// upward, so we flip.
fn facade_texture() -> Image {
    procedural(128, |u, v| {
        let v = 1.0 - v;
        let wall = [236, 233, 228, 255];
        if in_rect(u, v, 0.24, 0.24, 0.76, 0.82) {
            if in_rect(u, v, 0.27, 0.27, 0.73, 0.79) {
                // Glass: darker toward the bottom, a soft sky reflection on top.
                let t = (v - 0.27) / 0.52;
                let c = (60.0 + 70.0 * t) as u8;
                let mullion = (u - 0.5).abs() < 0.012;
                if mullion { [210, 210, 205, 255] } else { [c / 2 + 20, c / 2 + 35, c + 30, 255] }
            } else {
                [250, 250, 248, 255] // frame
            }
        } else if in_rect(u, v, 0.21, 0.19, 0.79, 0.24) {
            [205, 200, 192, 255] // sill
        } else if v < 0.04 {
            [215, 210, 202, 255] // floor band
        } else {
            wall
        }
    })
}

fn curtain_wall_texture() -> Image {
    procedural(128, |u, v| {
        let v = 1.0 - v;
        let frame = (u < 0.04 || u > 0.96) || (v < 0.06);
        if frame {
            [170, 178, 186, 255]
        } else {
            let t = v;
            [(150.0 + 60.0 * t) as u8, (175.0 + 50.0 * t) as u8, (200.0 + 40.0 * t) as u8, 255]
        }
    })
}

fn storefront_texture() -> Image {
    procedural(128, |u, v| {
        let v = 1.0 - v;
        if v < 0.06 {
            [120, 118, 112, 255] // kick plate
        } else if in_rect(u, v, 0.06, 0.06, 0.94, 0.78) {
            let door = in_rect(u, v, 0.40, 0.06, 0.60, 0.74);
            if door {
                [70, 62, 55, 255]
            } else {
                // Warm interior glow behind glass.
                let t = (v - 0.06) / 0.72;
                [(90.0 + 50.0 * t) as u8, (85.0 + 40.0 * t) as u8, (80.0 + 30.0 * t) as u8, 255]
            }
        } else if v > 0.86 {
            [236, 233, 228, 255] // sign band (tinted by wall color)
        } else {
            [225, 222, 216, 255]
        }
    })
}

fn setup_materials(
    mut commands: Commands,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut terrain_mats: ResMut<Assets<crate::ground_material::TerrainMaterial>>,
    mut scatter_mats: ResMut<Assets<crate::ground_material::ScatterMaterial>>,
) {
    let (terrain, ground) = crate::ground_material::create(&mut images, &mut terrain_mats, &mut scatter_mats);
    commands.insert_resource(ground);
    let facade_tex = images.add(facade_texture());
    let curtain_tex = images.add(curtain_wall_texture());
    let shop_tex = images.add(storefront_texture());
    let matte = |mats: &mut Assets<StandardMaterial>, rough: f32| {
        mats.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: rough, reflectance: 0.3, ..default() })
    };
    let wm = WorldMaterials {
        terrain,
        facade: mats.add(StandardMaterial {
            base_color_texture: Some(facade_tex),
            perceptual_roughness: 0.8,
            reflectance: 0.35,
            ..default()
        }),
        glass: mats.add(StandardMaterial {
            base_color_texture: Some(curtain_tex),
            perceptual_roughness: 0.12,
            metallic: 0.25,
            reflectance: 0.7,
            ..default()
        }),
        storefront: mats.add(StandardMaterial {
            base_color_texture: Some(shop_tex),
            perceptual_roughness: 0.4,
            reflectance: 0.5,
            ..default()
        }),
        matte: matte(&mut mats, 0.88),
        water: mats.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.45, 0.55),
            perceptual_roughness: 0.06,
            reflectance: 0.6,
            ..default()
        }),
        ocean: mats.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.32, 0.45),
            perceptual_roughness: 0.08,
            reflectance: 0.55,
            ..default()
        }),
    };
    commands.insert_resource(wm);
}
