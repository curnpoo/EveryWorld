//! Probe GI: a camera-centred, terrain-following grid of light probes
//! (`ed_world::probes`) uploaded as an irradiance volume. The patched
//! `vendor/bevy_pbr` shaders read it as per-face sky visibility + bounce
//! albedo and combine it with the atmosphere env map (see `probe_gi` in
//! `irradiance_volume.wesl`).
//!
//! Pipeline: gather a 2.5D [`SceneField`] around the camera a few thousand
//! cells per frame (terrain + albedo from `GroundCover` tiles, roofs from a
//! downward physics ray), bake on the async pool, upload a 3D texture.
//!
//! Knobs: `EW_PROBE_GI=0` off, `EW_PROBE_K` bounce multiplier,
//! `EW_PROBE_DEBUG=1|2` initial debug mode. The pause menu's Debug tab and F3
//! change the live settings: off -> per-face probe colours (unlit) -> lit
//! white spheres (shaded by the volume).

use crate::ground_scatter::GroundCover;
use crate::menu::{DebugSettings, ProbeView};
use crate::origin::WorldOrigin;
use crate::sky::Sun;
use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::light::{IrradianceVolume, LightProbe, NotShadowCaster};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future};
use ed_world::probes::{self, FACES, ProbeParams, ProbeVolume, SceneField};
use std::sync::Arc;

/// Gather cell size (m). The probe grid is sparse, so 128 m terrain cells keep
/// the 20 km-radius bounce field affordable.
const CELL: f32 = 128.0;
/// Scene margin beyond the probe grid (m), so edge probes see occluders.
const MARGIN: f32 = 256.0;
/// Cells gathered per frame.
const CELLS_PER_FRAME: usize = 3000;
/// Re-centre when the camera moves this far from the volume centre (m).
const RECENTER: f32 = 28.0;
/// Re-bake at least this often (s): tiles/buildings keep streaming in.
const REFRESH_S: f32 = 10.0;
/// Roof albedo (no per-building colour here).
const ROOF_ALBEDO: [f32; 3] = [0.32, 0.30, 0.28];
/// Debug spheres within this distance of the camera.
const DEBUG_RADIUS: f32 = 1_024.0;
/// Cast the roof/ground probe down from high enough for mountain-scale terrain.
const PROBE_TOP: f32 = 12_000.0;

pub struct ProbeGiPlugin;

impl Plugin for ProbeGiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugSettings>()
            .init_resource::<ProbeGi>()
            .add_systems(Update, (debug_toggle, sync_debug, drive_probes, update_intensity, debug_spheres).chain());
    }
}

struct Gather {
    center: Vec3,
    generation: u64,
    field: SceneField,
    next: usize,
}

#[derive(Resource, Default)]
pub(crate) struct ProbeGi {
    gather: Option<Gather>,
    task: Option<Task<(Vec3, u64, ProbeVolume)>>,
    /// Centre + origin generation of the live volume.
    pub(crate) live: Option<(Vec3, u64)>,
    pub(crate) volume: Option<Arc<ProbeVolume>>,
    entity: Option<Entity>,
    image: Option<Handle<Image>>,
    since_bake: f32,
    enabled: bool,
    /// 0 off, 1 probe colours, 2 lit spheres.
    debug: u8,
    debug_dirty: bool,
    debug_entity: Option<Entity>,
    pub(crate) bounce_k: f32,
    /// Sun direction the live/pending volume was baked for.
    baked_sun: Vec3,
}

fn params(sun: Vec3) -> ProbeParams {
    ProbeParams { sun, ..ProbeParams::default() }
}

fn snap(p: Vec3) -> Vec3 {
    let s = 8.0;
    Vec3::new((p.x / s).round() * s, 0.0, (p.z / s).round() * s)
}

fn camera_pos(cams: &Query<(&GlobalTransform, &Camera), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>) -> Option<Vec3> {
    // The native renderer deactivates the Bevy 3D camera, so fall back to any 3D camera.
    cams.iter().find(|(_, c)| c.is_active).or_else(|| cams.iter().next()).map(|(t, _)| t.translation())
}

#[allow(clippy::too_many_arguments)]
fn drive_probes(
    mut st: ResMut<ProbeGi>,
    time: Res<Time>,
    origin: Res<WorldOrigin>,
    cams: Query<(&GlobalTransform, &Camera), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    tiles: Query<(&GlobalTransform, &GroundCover)>,
    sunq: Query<&GlobalTransform, With<Sun>>,
    spatial: SpatialQuery,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    if !st.enabled {
        return;
    }
    st.since_bake += time.delta_secs();
    let Some(cam) = camera_pos(&cams) else { return };
    let generation = origin.generation;
    let sun = sunq.iter().next().map(|t| t.back().as_vec3()).unwrap_or(Vec3::ZERO);

    // Origin rebase: world coordinates jumped; old data is in the wrong place.
    if st.live.is_some_and(|(_, g)| g != generation) {
        if let Some(e) = st.entity.take() {
            commands.entity(e).despawn();
        }
        st.live = None;
        st.volume = None;
        st.debug_dirty = true;
    }
    if st.gather.as_ref().is_some_and(|g| g.generation != generation) {
        st.gather = None;
    }

    // Finished bake -> upload.
    if let Some(task) = st.task.as_mut() {
        if let Some((center, g, vol)) = block_on(future::poll_once(task)) {
            st.task = None;
            if g == generation {
                upload(&mut st, center, g, vol, &mut images, &mut commands);
            }
        }
        return;
    }

    // Gather in progress.
    if let Some(gather) = st.gather.as_mut() {
        gather_cells(gather, &tiles, &spatial);
        if gather.next >= gather.field.nx * gather.field.nz {
            let gather = st.gather.take().unwrap();
            let baked_for = st.baked_sun;
            let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(2).max(1);
            st.task = Some(AsyncComputeTaskPool::get().spawn(async move {
                let vol = probes::bake(&gather.field, &params(baked_for), threads);
                (gather.center, gather.generation, vol)
            }));
        }
        return;
    }

    // Start a new gather?
    if tiles.is_empty() {
        return;
    }
    let target = snap(cam);
    let need = match st.live {
        None => true,
        Some((c, _)) => {
            let d = Vec2::new(cam.x - c.x, cam.z - c.z).length();
            // Re-bake as the sun moves (~4 degrees) so bounce light follows the shadows.
            d > RECENTER || st.since_bake > REFRESH_S || st.baked_sun.dot(sun) < 0.9976
        }
    };
    if !need {
        return;
    }
    st.baked_sun = sun;
    let p = params(sun);
    let half = Vec2::new(p.nx as f32, p.nz as f32) * p.spacing * 0.5 + MARGIN;
    let nx = (2.0 * half.x / CELL).ceil() as usize;
    let nz = (2.0 * half.y / CELL).ceil() as usize;
    let origin = -half + Vec2::splat(CELL * 0.5);
    st.since_bake = 0.0;
    st.gather = Some(Gather { center: target, generation, field: SceneField::new(origin, CELL, nx, nz), next: 0 });
}

/// Fill the next batch of scene cells. Field coords are relative to `center`.
fn gather_cells(g: &mut Gather, tiles: &Query<(&GlobalTransform, &GroundCover)>, spatial: &SpatialQuery) {
    let n = g.field.nx * g.field.nz;
    let end = (g.next + CELLS_PER_FRAME).min(n);
    let inv: Vec<(bevy::math::Affine3A, bevy::math::Affine3A, &GroundCover)> =
        tiles.iter().map(|(t, c)| (t.affine(), t.affine().inverse(), c)).collect();
    let filter = SpatialQueryFilter::default();
    let mut last_tile = 0usize;
    for idx in g.next..end {
        let (i, k) = (idx % g.field.nx, idx / g.field.nx);
        let c = g.field.cell_center(i, k);
        let w = Vec3::new(g.center.x + c.x, 0.0, g.center.z + c.y);

        // Terrain + albedo from the tile containing this point (try the last hit first).
        let mut ground = None;
        for o in 0..inv.len() {
            let ti = (last_tile + o) % inv.len();
            let (fwd, back, cover) = &inv[ti];
            let src = &cover.0;
            let (u, v) = src.approx_uv(back.transform_point3(w));
            if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
                continue;
            }
            let h = src.grid.sample(u, v);
            let wp = fwd.transform_point3(src.proj.local(u, v, h as f64));
            let col = src.surface.ground_color(u, v, h, 0.0);
            ground = Some((wp.y, [col[0], col[1], col[2]]));
            last_tile = ti;
            break;
        }

        // Top of solid: terrain or roof, from the colliders.
        let top = spatial
            .cast_ray(Vec3::new(w.x, g.center.y + PROBE_TOP, w.z), Dir3::NEG_Y, PROBE_TOP * 2.0, true, &filter)
            .map(|hit| g.center.y + PROBE_TOP - hit.distance);

        let (gy, albedo) = match (ground, top) {
            (Some(gr), _) => gr,
            (None, Some(t)) => (t, [0.2; 3]),
            (None, None) => (cam_fallback(&g.field, idx), [0.2; 3]),
        };
        let top = top.unwrap_or(gy).max(gy);
        let roof = top > gy + 1.5;
        g.field.ground[idx] = gy - g.center.y;
        g.field.top[idx] = top - g.center.y;
        g.field.albedo[idx] = if roof { ROOF_ALBEDO } else { albedo };
    }
    g.next = end;
}

/// No tile and no collider here (not streamed yet): reuse the previous cell.
fn cam_fallback(f: &SceneField, idx: usize) -> f32 {
    if idx > 0 { f.ground[idx - 1] } else { 0.0 }
}

fn upload(st: &mut ProbeGi, center: Vec3, generation: u64, vol: ProbeVolume, images: &mut Assets<Image>, commands: &mut Commands) {
    let (w, h, d) = vol.dims();
    let mut bytes = Vec::with_capacity(w * h * d * 8);
    for t in &vol.texels {
        for c in t {
            bytes.extend_from_slice(&f32_to_f16(*c).to_le_bytes());
        }
    }
    let mut image = Image::new(
        Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: d as u32 },
        TextureDimension::D3,
        bytes,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    let handle = images.add(image);
    if let Some(old) = st.image.replace(handle.clone()) {
        images.remove(&old);
    }

    // Volume-local frame is world minus `center` (axis-aligned).
    let size = vol.box_max - vol.box_min;
    let mid = center + (vol.box_max + vol.box_min) * 0.5;
    let transform = Transform::from_translation(mid).with_scale(size);
    let gi = IrradianceVolume { voxels: handle, intensity: st.bounce_k.max(1.0), affects_lightmapped_meshes: false };
    // Small falloff: fade over the outer ~6% so re-centring isn't a hard pop at the border.
    let probe = LightProbe { falloff: Vec3::new(0.06, 0.02, 0.06) };
    match st.entity {
        Some(e) => {
            commands.entity(e).insert((gi, probe, transform));
        }
        None => {
            st.entity = Some(commands.spawn((gi, probe, transform, Visibility::default(), Name::new("probe gi"))).id());
        }
    }
    st.live = Some((center, generation));
    st.volume = Some(Arc::new(vol));
    st.debug_dirty = true;
}

/// Bounce strength follows the sun: sunlit ground is several times brighter
/// than the sky's up-irradiance at midday, about equal at dusk.
fn update_intensity(mut st: ResMut<ProbeGi>, sun: Query<&GlobalTransform, With<Sun>>, mut vols: Query<&mut IrradianceVolume>) {
    let elev = sun.iter().next().map(|t| t.back().y).unwrap_or(0.5);
    let user = std::env::var("EW_PROBE_K").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
    let k = if st.enabled { (1.0 + 4.0 * elev.max(0.0)) * user } else { 0.0 };
    st.bounce_k = k;
    let Some(e) = st.entity else { return };
    if let Ok(mut v) = vols.get_mut(e) {
        if (v.intensity - k).abs() > 1e-3 {
            v.intensity = k;
        }
    }
}

/// Copy the menu state into the bake/debug resources. The first run also
/// applies `EW_PROBE_GI=0` and `EW_PROBE_DEBUG` startup values.
fn sync_debug(debug: Res<DebugSettings>, mut st: ResMut<ProbeGi>, mut last: Local<Option<(bool, ProbeView)>>) {
    let now = (debug.probe_gi, debug.probe_view);
    if *last == Some(now) {
        return;
    }
    *last = Some(now);
    let was_enabled = st.enabled;
    st.enabled = debug.probe_gi;
    if st.enabled && !was_enabled {
        // Re-bake after enabling so a volume from before the toggle is not reused
        // after the world, sun or camera has moved.
        st.since_bake = REFRESH_S + 1.0;
        st.gather = None;
    }
    st.debug = match debug.probe_view {
        ProbeView::Off => 0,
        ProbeView::Colors => 1,
        ProbeView::Lit => 2,
    };
    st.debug_dirty = true;
}

/// F3 cycles probe spheres. I does the same while live diagnostics are on.
fn debug_toggle(keys: Res<ButtonInput<KeyCode>>, gfx: Res<crate::menu::GraphicsSettings>, mut settings: ResMut<DebugSettings>) {
    if keys.just_pressed(KeyCode::F3) || (gfx.show_stats && keys.just_pressed(KeyCode::KeyI)) {
        settings.probe_view = settings.probe_view.next();
        info!("probe GI debug: {}", settings.probe_view.label());
    }
}

#[derive(Component)]
struct ProbeDebug;

/// Merged icospheres at probe positions near the camera. Colours: each vertex
/// blends the six faces by N^2, showing `vis * sky + bounce * K` (unlit mode).
#[allow(clippy::too_many_arguments)]
fn debug_spheres(
    mut st: ResMut<ProbeGi>,
    cams: Query<(&GlobalTransform, &Camera), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    if !st.debug_dirty {
        return;
    }
    st.debug_dirty = false;
    if let Some(e) = st.debug_entity.take() {
        commands.entity(e).despawn();
    }
    let (Some(vol), Some((center, _)), Some(cam)) = (st.volume.clone(), st.live, camera_pos(&cams)) else { return };
    if st.debug == 0 || !st.enabled {
        return;
    }
    let unlit = st.debug == 1;
    let k = st.bounce_k;
    let sky = Vec3::new(0.45, 0.62, 1.0);
    let unit = Sphere::new(1.0).mesh().ico(1).unwrap();
    let up: Vec<[f32; 3]> = unit.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap().to_vec();
    let tris: Vec<u32> = unit.indices().unwrap().iter().map(|i| i as u32).collect();
    let (mut pos, mut nrm, mut col, mut idx) = (vec![], vec![], vec![], vec![]);
    let local_cam = cam - center;
    for pr in &vol.probes {
        let dist = pr.pos.distance(local_cam);
        if dist > DEBUG_RADIUS {
            continue;
        }
        let r = (0.3 + dist * 0.004).min(4.0);
        let base = pos.len() as u32;
        for v in &up {
            let n = Vec3::from(*v);
            pos.push((center + pr.pos + n * r).to_array());
            nrm.push(n.to_array());
            let c = if unlit {
                let mut acc = Vec3::ZERO;
                for (fi, f) in FACES.iter().enumerate() {
                    let w = n.dot(*f).max(0.0).powi(2);
                    let t = pr.faces[fi];
                    acc += w * (sky * t[3] + Vec3::new(t[0], t[1], t[2]) * k);
                }
                // Mark probes that were buried and filled from neighbours.
                // Reinhard so bright (K ~ 4 at noon) probes don't all clip to white.
                acc = acc * 0.6 / (Vec3::ONE + acc * 0.6);
                if pr.filled { acc = acc * 0.5 + Vec3::new(0.5, 0.0, 0.5); }
                acc
            } else {
                Vec3::ONE
            };
            col.push([c.x, c.y, c.z, 1.0]);
        }
        idx.extend(tris.iter().map(|i| base + i));
    }
    if pos.is_empty() {
        return;
    }
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, crate::native::world_mesh_usage())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx));
    let mat = StandardMaterial { base_color: Color::WHITE, unlit, perceptual_roughness: 1.0, reflectance: 0.0, ..default() };
    let e = commands
        .spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mats.add(mat)), Transform::IDENTITY, NotShadowCaster, ProbeDebug, Name::new("probe gi debug")))
        .id();
    st.debug_entity = Some(e);
}

/// f32 -> IEEE half (round-to-nearest, subnormals flushed; values here are small).
pub(crate) fn f32_to_f16(v: f32) -> u16 {
    let b = v.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32 - 127 + 15;
    if v.is_nan() {
        return 0x7e00;
    }
    if exp <= 0 {
        return sign;
    }
    if exp >= 31 {
        return sign | 0x7c00;
    }
    let mant = b & 0x7f_ffff;
    let mut h = sign as u32 | ((exp as u32) << 10) | (mant >> 13);
    if mant & 0x1000 != 0 {
        h += 1; // round (may carry into exponent, which is still correct)
    }
    h as u16
}

#[cfg(test)]
mod tests {
    use super::f32_to_f16;
    #[test]
    fn half() {
        assert_eq!(f32_to_f16(0.0), 0);
        assert_eq!(f32_to_f16(1.0), 0x3c00);
        assert_eq!(f32_to_f16(-2.0), 0xc000);
        assert_eq!(f32_to_f16(0.5), 0x3800);
    }
}
