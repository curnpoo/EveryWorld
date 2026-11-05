//! Shared materials and procedural textures. Every material in the world
//! is defined here so art direction lives in one place.

use bevy::prelude::*;

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
    /// Untextured, vertex-colored, matte (roofs, roads, trees, props, rocks).
    pub matte: Handle<StandardMaterial>,
    /// Matte material whose color comes from the global palette via UV
    /// (for meshlet meshes, which cannot carry vertex color).
    pub palette_matte: Handle<StandardMaterial>,
    pub water: Handle<StandardMaterial>,
    pub ocean: Handle<StandardMaterial>,
}

fn setup_materials(
    mut commands: Commands,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut terrain_mats: ResMut<Assets<crate::ground_material::TerrainMaterial>>,
    mut scatter_mats: ResMut<Assets<crate::ground_material::ScatterMaterial>>,
) {
    let pool = crate::ground_material::SatPool::new(&mut images);
    let far = crate::far_shadows::FarPool::new(&mut images);
    let (terrain, ground) = crate::ground_material::create(&mut images, &mut terrain_mats, &mut scatter_mats, &pool, &far);
    commands.insert_resource(pool);
    commands.insert_resource(far);
    commands.insert_resource(ground);
    let matte = |mats: &mut Assets<StandardMaterial>, rough: f32| {
        mats.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: rough, reflectance: 0.3, ..default() })
    };
    let wm = WorldMaterials {
        terrain,
        matte: matte(&mut mats, 0.88),
        palette_matte: mats.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(images.add(palette_image())),
            perceptual_roughness: 0.88,
            reflectance: 0.3,
            ..default()
        }),
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

/// The global color lookup texture (see `ed_world::mesh::palette_uv`).
fn palette_image() -> Image {
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let side = ed_world::mesh::PALETTE_SIDE;
    let mut data = Vec::with_capacity((side * side * 4) as usize);
    for i in 0..side * side {
        data.extend_from_slice(&ed_world::mesh::palette_texel(i));
    }
    let mut img = Image::new(Extent3d { width: side, height: side, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, bevy::asset::RenderAssetUsages::RENDER_WORLD);
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        mipmap_filter: ImageFilterMode::Nearest,
        ..default()
    });
    img
}
