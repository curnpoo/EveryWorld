//! Native Metal world renderer bridge (default on macOS; `EW_NATIVE=0` for Bevy).
//!
//! Bevy keeps simulation, streaming, physics, input and UI. The 3D camera is switched off; every visible
//! opaque `Mesh3d` is mirrored into `ed_metal::world::WorldRenderer`, which draws into a `CAMetalLayer`
//! sitting underneath Bevy's (transparent) window. Bevy's UI camera draws on top.

use bevy::prelude::*;

pub fn enabled() -> bool {
    // Default on for macOS; `EW_NATIVE=0` falls back to the Bevy renderer.
    cfg!(target_os = "macos") && std::env::var_os("EW_NATIVE").is_none_or(|v| v != "0")
}

/// Keep CPU copies of meshes in the main world only when the native renderer needs them.
pub fn mesh_usage() -> bevy::asset::RenderAssetUsages {
    if enabled() {
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD
    } else {
        bevy::asset::RenderAssetUsages::RENDER_WORLD
    }
}

pub struct NativePlugin;

#[cfg(not(target_os = "macos"))]
impl Plugin for NativePlugin {
    fn build(&self, _app: &mut App) {}
}

#[cfg(target_os = "macos")]
impl Plugin for NativePlugin {
    fn build(&self, app: &mut App) {
        if !enabled() {
            return;
        }
        app.insert_non_send(imp::NativeState::default())
            .add_systems(Update, (imp::manage_cameras, imp::send_atlas))
            .add_systems(Last, imp::native_frame);
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::impostors::ImpostorMaterial;
    use bevy::camera::visibility::InheritedVisibility;
    use bevy::window::{PrimaryWindow, RawHandleWrapper};
    use ed_metal::world::{Blend, DrawItem, FrameParams, WorldRenderer};
    use raw_window_handle::RawWindowHandle;
    use std::collections::HashMap;
    use std::time::Instant;

    /// Hand the baked impostor atlas to the Metal renderer once it exists.
    pub fn send_atlas(mut st: NonSendMut<NativeState>, imps: Option<Res<crate::impostors::Impostors>>, mats: Res<Assets<ImpostorMaterial>>, images: Res<Assets<Image>>) {
        let st = &mut *st;
        let (Some(r), Some(imps)) = (st.renderer.as_mut(), imps) else { return };
        if r.has_atlas() {
            return;
        }
        let Some(m) = mats.get(&imps.material) else { return };
        let (Some(a), Some(n)) = (images.get(&m.extension.albedo), images.get(&m.extension.normal)) else { return };
        let (Some(ad), Some(nd)) = (&a.data, &n.data) else { return };
        let sz = a.texture_descriptor.size;
        if sz.depth_or_array_layers <= 1 {
            return; // still the 1x1 placeholder
        }
        r.upload_impostor_atlas(sz.width as usize, sz.depth_or_array_layers as usize, a.texture_descriptor.mip_level_count as usize, ad, nd);
        info!("native: impostor atlas uploaded ({} layers)", sz.depth_or_array_layers);
    }

    #[derive(Default)]
    pub struct NativeState {
        renderer: Option<WorldRenderer>,
        tried: bool,
        ids: HashMap<AssetId<Mesh>, u64>,
        next_id: u64,
        frame: u64,
        frames_since_report: u32,
        last_report: Option<Instant>,
        draw_scratch: Vec<DrawItem>,
        skipped_no_cpu: u32,
        skipped_tex: u32,
        tex_ids: std::collections::HashMap<AssetId<Image>, u64>,
        idle_frames: u32,
        shot_done: Option<u64>,
        ground_sent: bool,
    }

    /// Replaces `render_scale::sync_cameras` while native: 3D camera off, UI camera owns the window.
    pub fn manage_cameras(
        state: Res<State<crate::menu::GameState>>,
        mut cams3d: Query<(Entity, &mut Camera, Has<bevy::light::AtmosphereEnvironmentMapLight>, Has<IsDefaultUiCamera>), (With<Camera3d>, Without<crate::render_scale::UiCamera>, Without<crate::globe::GlobeCamera>)>,
        mut ui: Query<(Entity, &mut Camera, Has<IsDefaultUiCamera>), (With<crate::render_scale::UiCamera>, Without<Camera3d>)>,
        mut commands: Commands,
    ) {
        for (e, mut c, has_probe, is_ui) in &mut cams3d {
            if c.is_active {
                c.is_active = false;
            }
            if has_probe {
                commands.entity(e).remove::<bevy::light::AtmosphereEnvironmentMapLight>();
            }
            if is_ui {
                commands.entity(e).remove::<IsDefaultUiCamera>();
            }
        }
        for (e, mut c, is_ui) in &mut ui {
            if !c.is_active {
                c.is_active = true;
            }
            let want = if std::env::var_os("EW_NATIVE_MAGENTA").is_some() { ClearColorConfig::Custom(Color::srgba(1.0, 0.0, 1.0, 0.5)) } else { ClearColorConfig::Custom(Color::NONE) };
            if !matches!(&c.clear_color, ClearColorConfig::Custom(col) if matches!(&want, ClearColorConfig::Custom(w) if w == col)) {
                c.clear_color = want;
            }
            if !is_ui {
                commands.entity(e).insert(IsDefaultUiCamera);
            }
        }
    }

    fn frustum_planes(m: Mat4) -> [Vec4; 6] {
        let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
        let mut p = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2];
        for pl in &mut p {
            *pl /= pl.truncate().length().max(1e-9);
        }
        p
    }

    #[allow(clippy::too_many_arguments)]
    pub fn native_frame(
        mut st: NonSendMut<NativeState>,
        windows: Query<(&Window, &RawHandleWrapper), With<PrimaryWindow>>,
        cfg: Res<crate::config::Config>,
        mgr: Res<crate::streaming::TileManager>,
        mut exit: MessageWriter<AppExit>,
        game: Res<State<crate::menu::GameState>>,
        cams: Query<(&GlobalTransform, &Projection), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
        sun: Query<&GlobalTransform, With<crate::sky::Sun>>,
        meshes: Res<Assets<Mesh>>,
        std_mats: Res<Assets<StandardMaterial>>,
        fol_mats: Res<Assets<crate::foliage::FoliageMaterial>>,
        scat_mats: Res<Assets<crate::ground_material::ScatterMaterial>>,
        images: Res<Assets<Image>>,
        sat: Option<Res<crate::ground_material::SatPool>>,
        gfx: Res<crate::menu::GraphicsSettings>,
        items: Query<(&Mesh3d, &GlobalTransform, &InheritedVisibility, Option<&MeshMaterial3d<StandardMaterial>>, Option<&bevy::mesh::MeshTag>, Has<MeshMaterial3d<crate::ground_material::TerrainMaterial>>, Has<bevy::light::NotShadowCaster>, Has<crate::boot_plume::PlumeMesh>, Has<crate::body::WingMesh>, Option<&MeshMaterial3d<crate::foliage::FoliageMaterial>>, Option<&MeshMaterial3d<crate::ground_material::ScatterMaterial>>, Has<MeshMaterial3d<ImpostorMaterial>>), (Without<MeshMaterial3d<crate::clouds::CloudMaterial>>)>,
    ) {
        let st = &mut *st;
        let headless = cfg.headless;
        let window = windows.single().ok();
        if st.renderer.is_none() {
            if st.tried {
                return;
            }
            if !headless && window.is_none() {
                return;
            }
            st.tried = true;
            if headless {
                st.renderer = WorldRenderer::new_headless();
            } else if let Some((_, handle)) = window {
                if let RawWindowHandle::AppKit(h) = handle.get_window_handle() {
                    st.renderer = WorldRenderer::new(h.ns_view.as_ptr());
                }
            }
            match &st.renderer {
                Some(_) => info!("native: Metal world renderer attached{}", if headless { " (headless, offscreen)" } else { "" }),
                None => error!("native: failed to attach Metal world renderer"),
            }
        }
        let Some(r) = st.renderer.as_mut() else { return };
        if !st.ground_sent && std::env::var_os("EW_NATIVE_NOGROUND").is_none() {
            if let Some(g) = crate::ground_material::GROUND_CPU.get() {
                let al: Vec<&[u8]> = g.albedo.iter().map(|v| v.as_slice()).collect();
                let nr: Vec<&[u8]> = g.normal.iter().map(|v| v.as_slice()).collect();
                r.upload_ground(g.size, &al, &nr, &g.macro_tex, g.macro_size, &g.sizes, &g.bump, 1.0);
                st.ground_sent = true;
                info!("native: ground layers uploaded");
            }
        }
        let menu = matches!(*game.get(), crate::menu::GameState::Menu | crate::menu::GameState::Paused);
        r.set_hidden(menu);
        if menu {
            return;
        }
        let Some((cam_gt, proj)) = cams.iter().next() else { return };
        let Projection::Perspective(p) = proj else { return };
        let (w, h) = match window {
            Some((win, _)) if !headless => (win.physical_width().max(1), win.physical_height().max(1)),
            _ => (cfg.width.max(1), cfg.height.max(1)),
        };
        let aspect = w as f32 / h as f32;
        let view = cam_gt.to_matrix().inverse();
        let raster = Mat4::perspective_infinite_reverse_rh(p.fov, aspect, p.near.max(0.1)) * view;
        let planes = frustum_planes(Mat4::perspective_rh(p.fov, aspect, p.near.max(0.1), 120_000.0) * view);
        let cam_pos = cam_gt.translation();

        // Upload newly seen meshes/textures (bounded per frame), build the draw list.
        st.draw_scratch.clear();
        let mut uploads = 0;
        let mut tex_uploads = 0;
        for (m3, gt, vis, mat, mtag, is_terrain, no_cast, plume, wing, fol, scat, is_imp) in &items {
            let dynamic = plume || wing;
            if !vis.get() {
                continue;
            }
            let (mut tint, mut emissive, mut rough, mut metal, mut cutoff, mut unlit, mut blend, mut double_sided) = ([1.0f32; 4], [0.0f32; 3], 0.8f32, 0.0f32, 0.5f32, false, Blend::Opaque, false);
            let mut tex = 0u64;
            let mut tex2 = 0u64;
            let base_mat = mat
                .and_then(|m| std_mats.get(&m.0))
                .or_else(|| fol.and_then(|m| fol_mats.get(&m.0)).map(|e| &e.base))
                .or_else(|| scat.and_then(|m| scat_mats.get(&m.0)).map(|e| &e.base));
            if let Some(sm) = base_mat {
                let c = sm.base_color.to_linear();
                tint = [c.red, c.green, c.blue, c.alpha];
                let e = sm.emissive;
                emissive = [e.red, e.green, e.blue];
                rough = sm.perceptual_roughness;
                metal = sm.metallic;
                unlit = sm.unlit;
                double_sided = sm.double_sided;
                match sm.alpha_mode {
                    AlphaMode::Opaque => {}
                    AlphaMode::Mask(c) => {
                        blend = Blend::Mask;
                        cutoff = c;
                    }
                    _ => blend = Blend::Alpha,
                }
                if let Some(img) = &sm.base_color_texture {
                    let key = img.id();
                    let id = match st.tex_ids.get(&key) {
                        Some(i) => *i,
                        None => {
                            if tex_uploads < 8 {
                                let id = st.next_id + 1;
                                st.next_id = id;
                                st.tex_ids.insert(key, id);
                                tex_uploads += 1;
                                if let Some(im) = images.get(img) {
                                    let ok = matches!(im.texture_descriptor.format, bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb | bevy::render::render_resource::TextureFormat::Rgba8Unorm)
                                        && im.texture_descriptor.dimension == bevy::render::render_resource::TextureDimension::D2
                                        && im.texture_descriptor.size.depth_or_array_layers == 1;
                                    match (&im.data, ok) {
                                        (Some(d), true) => r.upload_texture(id, im.width() as usize, im.height() as usize, d, im.texture_descriptor.format == bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb),
                                        _ => st.skipped_tex += 1,
                                    }
                                }
                                id
                            } else {
                                0
                            }
                        }
                    };
                    if r.has_texture(id) {
                        tex = id;
                    }
                }
                if blend == Blend::Opaque && tint[3] < 0.99 {
                    blend = Blend::Alpha;
                }
            }
            if is_imp {
                rough = 0.95;
                double_sided = true;
            }
            if is_terrain {
                rough = 0.95;
                if let (Some(pool), Some(t)) = (&sat, mtag) {
                    let key = ((t.value >> 24) << 12) | (t.value & 0xFFF);
                    if let Some((gen_id, size, data)) = pool.cpu.get(&key) {
                        let tid = (1u64 << 40) | *gen_id;
                        if !r.has_texture(tid) && tex_uploads < 8 {
                            r.upload_texture(tid, *size as usize, *size as usize, data, true);
                            tex_uploads += 1;
                        }
                        if r.has_texture(tid) {
                            tex = tid;
                        }
                        if let Some(det) = pool.cpu_detail.get(&key) {
                            let did = (1u64 << 41) | *gen_id;
                            if !r.has_texture(did) && tex_uploads < 8 {
                                r.upload_texture(did, *size as usize, *size as usize, det, false);
                                tex_uploads += 1;
                            }
                            if r.has_texture(did) {
                                tex2 = did;
                            }
                        }
                    }
                }
            }
            let id = *st.ids.entry(m3.0.id()).or_insert_with(|| {
                st.next_id += 1;
                st.next_id
            });
            // Dynamic meshes (the boot plume) are rebuilt on the CPU every frame: re-upload each time
            // and skip the draw while they are empty.
            if dynamic && meshes.get(&m3.0).is_none_or(|m| m.indices().is_none_or(|i| i.is_empty())) {
                continue;
            }
            if !r.has_mesh(id) || dynamic {
                if uploads >= 48 {
                    continue;
                }
                let Some(mesh) = meshes.get(&m3.0) else { continue };
                let (Some(pos), Some(idx)) = (mesh.try_attribute(Mesh::ATTRIBUTE_POSITION).ok().and_then(|a| a.as_float3()), mesh.try_indices().ok()) else {
                    st.skipped_no_cpu += 1;
                    continue;
                };
                let nor: Vec<[f32; 3]> = mesh.try_attribute(Mesh::ATTRIBUTE_NORMAL).ok().and_then(|a| a.as_float3()).map(|n| n.to_vec()).unwrap_or_default();
                let uv: Vec<[f32; 2]> = match mesh.try_attribute(Mesh::ATTRIBUTE_UV_0).ok() {
                    Some(bevy::mesh::VertexAttributeValues::Float32x2(u)) => u.clone(),
                    _ => Vec::new(),
                };
                let col: Vec<[f32; 4]> = match mesh.try_attribute(Mesh::ATTRIBUTE_COLOR).ok() {
                    Some(bevy::mesh::VertexAttributeValues::Float32x4(c)) => c.clone(),
                    _ => Vec::new(),
                };
                let idx: Vec<u32> = idx.iter().map(|i| i as u32).collect();
                r.upload(id, pos, &nor, &uv, &col, &idx);
                if is_terrain {
                    use bevy::mesh::VertexAttributeValues as V;
                    if let (Some(V::Unorm8x4(a)), Some(V::Unorm8x4(b)), Some(V::Unorm8x4(c))) = (
                        mesh.attribute(crate::ground_material::ATTRIBUTE_SPLAT0),
                        mesh.attribute(crate::ground_material::ATTRIBUTE_SPLAT1),
                        mesh.attribute(crate::ground_material::ATTRIBUTE_SPLAT2),
                    ) {
                        let w: Vec<[u8; 12]> = (0..a.len().min(b.len()).min(c.len()))
                            .map(|i| [a[i][0], a[i][1], a[i][2], a[i][3], b[i][0], b[i][1], b[i][2], b[i][3], c[i][0], c[i][1], c[i][2], c[i][3]])
                            .collect();
                        r.upload_splat(id, &w);
                    }
                }
                if is_imp {
                    use bevy::mesh::VertexAttributeValues as V;
                    if let (Some(V::Float32x4(pa)), Some(V::Float32x4(ax)), Some(V::Float32x2(tu))) = (
                        mesh.attribute(crate::impostors::ATTRIBUTE_IMP_PARAMS),
                        mesh.attribute(crate::impostors::ATTRIBUTE_IMP_AXIS),
                        mesh.attribute(crate::impostors::ATTRIBUTE_IMP_TILE_UV),
                    ) {
                        r.upload_extra(id, pa, ax, tu);
                    }
                }
                uploads += 1;
            }
            let Some(gm) = r.mesh(id) else { continue };
            let model = gt.to_matrix();
            let c = model.transform_point3(Vec3::from(gm.center));
            let s = gt.scale().max_element().max(1e-4);
            let rad = gm.radius * s;
            let visible = !planes.iter().any(|pl| pl.truncate().dot(c) + pl.w < -rad);
            st.draw_scratch.push(DrawItem {
                mesh: id,
                model: model.to_cols_array_2d(),
                tint,
                emissive,
                rough,
                metal,
                cutoff,
                unlit,
                tex,
                tex2,
                blend,
                double_sided,
                visible,
                cast_shadow: !no_cast,
                terrain: is_terrain,
                impostor: is_imp,
            });
        }

        let to_sun = sun.single().map(|g| -g.forward().as_vec3()).unwrap_or(Vec3::new(0.4, 0.8, 0.3).normalize());
        let day = (to_sun.y * 6.0).clamp(0.03, 1.0);
        let sky = Vec3::new(0.38, 0.55, 0.95) * day * 1.4;
        {
            use crate::menu::AaMode;
            let scale = if std::env::var_os("EW_NATIVE_SCALE").is_some() {
                r.render_scale
            } else if gfx.aa_mode == AaMode::Upscale {
                gfx.render_pct as f32 / 100.0
            } else {
                1.0
            };
            r.set_options(gfx.ambient_occlusion, gfx.bloom, gfx.aa_mode == AaMode::Aa, scale);
        }
        let fp = FrameParams {
                view_proj: raster.to_cols_array_2d(),
                cam_pos: cam_pos.to_array(),
                cam_right: cam_gt.right().as_vec3().to_array(),
                cam_up: cam_gt.up().as_vec3().to_array(),
                cam_fwd: cam_gt.forward().as_vec3().to_array(),
                tan_half_fov: (p.fov * 0.5).tan(),
                aspect,
                near: p.near.max(0.1),
                sun: [to_sun.x, to_sun.y, to_sun.z, day],
                sky: sky.to_array(),
                fog_density: 1.0 / 200_000.0,
                shadow_far: 700.0,
                terrain_shadow_far: 20_000.0,
                shadows: gfx.shadows && std::env::var_os("EW_NATIVE_NOSHADOW").is_none(),
                exposure: 1.7,
                size: (w, h),
                scale: window.map(|(w, _)| w.scale_factor() as f64).unwrap_or(1.0),
        };
        if headless {
            let pixels = r.render_offscreen(&fp, &st.draw_scratch);
            if mgr.idle() { st.idle_frames += 1 } else { st.idle_frames = 0 }
            if let (Some(path), Some(px)) = (&cfg.screenshot, pixels) {
                let ready = st.frame >= cfg.screenshot_frames as u64 && (st.idle_frames >= 20 || std::env::args().any(|a| a == "--no-wait"));
                if st.shot_done.is_none() && (ready || st.frame > 20_000) {
                    let img = Image::new(bevy::render::render_resource::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, bevy::render::render_resource::TextureDimension::D2, px, bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb, bevy::asset::RenderAssetUsages::MAIN_WORLD);
                    match img.try_into_dynamic().map(|d| d.save(path)) {
                        Ok(Ok(())) => info!("native: saved {}", path.display()),
                        e => error!("native: screenshot failed: {:?}", e.is_ok()),
                    }
                    st.shot_done = Some(st.frame);
                }
                if st.shot_done.is_some() {
                    exit.write(AppExit::Success);
                }
            }
        } else {
            r.render(&fp, &st.draw_scratch);
        }

        st.frame += 1;
        st.frames_since_report += 1;
        if st.frame % 240 == 0 {
            let live: std::collections::HashSet<u64> = meshes.iter().filter_map(|(id, _)| st.ids.get(&id).copied()).collect();
            r.retain_meshes(|k| live.contains(&k));
            st.ids.retain(|k, _| meshes.contains(*k));
            if let Some(pool) = &sat {
                let live: std::collections::HashSet<u64> = pool.cpu.values().flat_map(|v| [(1u64 << 40) | v.0, (1u64 << 41) | v.0]).collect();
                let used: std::collections::HashSet<u64> = st.tex_ids.values().copied().collect();
                r.retain_textures(|k| k < (1u64 << 40) && used.contains(&k) || live.contains(&k));
            }
        }
        let now = Instant::now();
        let last = *st.last_report.get_or_insert(now);
        if now.duration_since(last).as_secs_f32() >= 3.0 {
            let fps = st.frames_since_report as f32 / now.duration_since(last).as_secs_f32();
            let times = r.times.sorted().iter().map(|(n, ms)| format!("{n} {ms:.2} ms")).collect::<Vec<_>>().join(", ");
            info!("native: {fps:.0} fps | {} draws {:.2} M tris | {} meshes resident | gpu {times} | no-cpu-data {} no-cpu-tex {} | shadow draws {} (+{} cached)", r.draws, r.tris as f32 / 1e6, r.mesh_count(), st.skipped_no_cpu, st.skipped_tex, r.shadow_draws, r.shadow_skipped);
            st.frames_since_report = 0;
            st.last_report = Some(now);
            r.recalibrate();
        }
    }
}
