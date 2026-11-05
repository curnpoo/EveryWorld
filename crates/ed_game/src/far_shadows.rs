//! Far sun shadows past the last cascade (`docs/plans/shadows.md`, stage 2).
//! On by default; `EW_FAR_SHADOWS=0` turns it off, `EW_FAR_SHADOWS=debug`
//! tints far-shadowed terrain magenta to show where the terms act.
//!
//! Tiles bake a horizon map and a surface map on their worker thread
//! (`ed_world::farshadow`). This module keeps them in two fixed GPU texture
//! arrays that share one layer index per tile, like the satellite pool
//! (`ground_material::SatPool`). The layer + 1 rides in bits 12-22 of the
//! terrain entity's `MeshTag`. `terrain.wesl` does the shading.
//!
//! Memory is fixed: 328 layers of 67 KB (horizon) + 128 KB (surface) = 64 MB
//! when enabled, one dummy layer of each when not.

use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use ed_world::farshadow::{self, FarShadowTex, HZ_H, HZ_W, SURF_N};

use crate::ground_material::{LayerUpload, layer_array_image, write_layers};

/// Same total as the satellite classes: one far layer per resident terrain tile.
const LAYERS: u32 = 72 + 96 + 192 - 32;

/// `EW_FAR_SHADOWS=debug`: the terrain shader tints what the far terms darken.
pub static DEBUG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub struct FarShadowsPlugin;

impl Plugin for FarShadowsPlugin {
    fn build(&self, app: &mut App) {
        let v = std::env::var("EW_FAR_SHADOWS").unwrap_or_default();
        let on = v != "0";
        farshadow::set_enabled(on);
        DEBUG.store(v == "debug", std::sync::atomic::Ordering::Relaxed);
        app.add_systems(Update, sync_impostors);
        info!("far shadows {}: horizon maps, canopy darkening, surface ray march{}", if on { "on" } else { "off" }, if v == "debug" { " (debug tint)" } else { "" });
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            use bevy::render::{ExtractSchedule, Render, RenderSystems};
            render_app
                .init_resource::<RenderFarUploads>()
                .add_systems(ExtractSchedule, extract_far_uploads)
                .add_systems(Render, write_far_uploads.in_set(RenderSystems::PrepareResources));
        }
    }
}

/// Fixed pool of far-shadow layers shared by every terrain tile.
#[derive(Resource)]
pub struct FarPool {
    pub horizon: Handle<Image>,
    pub surface: Handle<Image>,
    free: Vec<u32>,
    uploads: Vec<LayerUpload>,
}

impl FarPool {
    pub fn new(images: &mut Assets<Image>) -> Self {
        let layers = if farshadow::enabled() { LAYERS } else { 1 };
        let n = SURF_N as u32;
        let horizon = images.add(layer_array_image(HZ_W, HZ_H, layers, TextureFormat::Rgba8Unorm));
        let surface = images.add(layer_array_image(n, n, layers, TextureFormat::Rgba16Float));
        let free = if farshadow::enabled() { (0..layers).rev().collect() } else { Vec::new() };
        Self { horizon, surface, free, uploads: Vec::new() }
    }

    /// Reserve a layer for `tex` and queue both uploads. Returns the bits to
    /// OR into the terrain `MeshTag` (0 = none: pool full or disabled).
    pub fn alloc(&mut self, tex: FarShadowTex) -> u32 {
        let Some(layer) = self.free.pop() else {
            if farshadow::enabled() {
                warn!("far-shadow pool full; tile drawn without far shadows");
            }
            return 0;
        };
        let n = SURF_N as u32;
        let surface: Vec<u8> = tex.surface.iter().flat_map(|h| h.to_le_bytes()).collect();
        self.uploads.push(LayerUpload { image: self.horizon.id(), layer, width: HZ_W, height: HZ_H, bytes_per_px: 4, data: tex.horizon });
        self.uploads.push(LayerUpload { image: self.surface.id(), layer, width: n, height: n, bytes_per_px: 8, data: surface });
        (layer + 1) << 12
    }

    /// Return the far layer packed in a terrain `MeshTag`, if any.
    pub fn release(&mut self, tag: u32) {
        let f = (tag >> 12) & 0x7FF; // bit 23: ground_material::DETAIL_BIT
        if f == 0 {
            return;
        }
        let layer = f - 1;
        let (h, s) = (self.horizon.id(), self.surface.id());
        self.uploads.retain(|u| !(u.layer == layer && (u.image == h || u.image == s)));
        self.free.push(layer);
    }
}

/// Tree impostors read the same horizon pool and Earth frame as terrain
/// (`impostor.wesl`); refresh when the origin re-anchors.
fn sync_impostors(
    origin: Res<crate::origin::WorldOrigin>,
    imp: Option<Res<crate::impostors::Impostors>>,
    pool: Option<Res<FarPool>>,
    mut mats: ResMut<Assets<crate::impostors::ImpostorMaterial>>,
    mut last: Local<Option<u64>>,
) {
    let (Some(imp), Some(pool)) = (imp, pool) else { return };
    if *last == Some(origin.generation) {
        return;
    }
    let Some(mut m) = mats.get_mut(&imp.material) else { return };
    let (c, p) = farshadow::earth_axes(origin.frame.ecef_to_render());
    let on = if farshadow::enabled() { 1.0 } else { 0.0 };
    let dbg = if DEBUG.load(std::sync::atomic::Ordering::Relaxed) { 1.0 } else { 0.0 };
    m.extension.params.far_c = c.as_vec3().extend(on);
    m.extension.params.far_p = p.as_vec3().extend(dbg);
    m.extension.far_horizon = pool.horizon.clone();
    *last = Some(origin.generation);
}

#[derive(Resource, Default)]
struct RenderFarUploads(Vec<LayerUpload>);

fn extract_far_uploads(mut main: ResMut<bevy::render::MainWorld>, mut out: ResMut<RenderFarUploads>) {
    if let Some(mut pool) = main.get_resource_mut::<FarPool>()
        && !pool.uploads.is_empty()
    {
        out.0.append(&mut pool.uploads);
    }
}

fn write_far_uploads(
    mut pending: ResMut<RenderFarUploads>,
    gpu_images: Res<bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>>,
    queue: Res<bevy::render::renderer::RenderQueue>,
) {
    write_layers(&mut pending.0, &gpu_images, &queue);
}
