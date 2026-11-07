//! 3D-only render resolution. When the percentage is below 100 the 3D camera
//! renders into a smaller offscreen image, which a full-screen UI image then
//! upscales over the window with FSR 1 EASU (`shaders/fsr_easu.wesl`), then
//! CAS on the 3D camera sharpens (RCAS's role). The UI stays at native res.
//! `EW_FSR=0` falls back to the plain bilinear stretch for A/B.

use crate::menu::GraphicsSettings;
use bevy::camera::RenderTarget;
use bevy::anti_alias::contrast_adaptive_sharpening::ContrastAdaptiveSharpening;
use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin};
use bevy::render::render_resource::TextureFormat;
use bevy::ui::IsDefaultUiCamera;
use bevy::window::PrimaryWindow;

pub struct RenderScalePlugin;

impl Plugin for RenderScalePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/fsr_easu.wesl");
        app.add_plugins(UiMaterialPlugin::<FsrMaterial>::default())
            .init_resource::<ScaleState>().add_systems(Startup, spawn_ui_camera).add_systems(Update, (apply_render_scale, sync_cameras));
    }
}

/// FSR 1 EASU upscale of the low-res 3D image (see `shaders/fsr_easu.wesl`).
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub struct FsrMaterial {
    #[texture(0)]
    pub source: Handle<Image>,
}

impl UiMaterial for FsrMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://everyworld/shaders/fsr_easu.wesl".into()
    }
}

fn fsr_enabled() -> bool {
    std::env::var("EW_FSR").map(|v| v != "0").unwrap_or(true)
}

#[derive(Resource, Default)]
struct ScaleState {
    size: UVec2,
    image: Option<Handle<Image>>,
    extras: Vec<Entity>,
}

#[derive(Component)]
pub struct UiCamera;

/// The window camera for UI. It is always present so the 3D camera can be
/// switched off entirely (main menu) without taking the UI with it.
fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Msaa::Off, Camera { order: 10, ..default() }, IsDefaultUiCamera, UiCamera));
}

/// The 3D camera owns the UI by itself in normal play (a second camera
/// sharing the window target corrupts the HDR output). The dedicated UI camera
/// only runs when the 3D camera is off (main menu, pause) or renders to a scaled image.
fn sync_cameras(
    state: Res<State<crate::menu::GameState>>,
    gfx: Res<GraphicsSettings>,
    mut cams3d: Query<(Entity, &mut Camera, Has<bevy::light::AtmosphereEnvironmentMapLight>, Has<IsDefaultUiCamera>), (With<Camera3d>, Without<UiCamera>, Without<crate::globe::GlobeCamera>)>,
    mut ui: Query<(Entity, &mut Camera, Has<IsDefaultUiCamera>), (With<UiCamera>, Without<Camera3d>)>,
    mut commands: Commands,
) {
    if crate::native::enabled() {
        return;
    }
    // Paused counts as menu: the pause screen covers the world with a 90%
    // panel, and rendering the full 3D scene under it next to the map globe
    // drops the map to ~10 fps (too slow to drag).
    let menu = matches!(*state.get(), crate::menu::GameState::Menu | crate::menu::GameState::Paused);
    let two_cam = menu || gfx.render_pct < 100;
    for (e, mut c, has_probe, is_ui) in &mut cams3d {
        if c.is_active == menu {
            c.is_active = !menu;
        }
        // The atmosphere env-map probe binds view uniforms that only exist
        // while the camera renders; leaving it on an inactive camera panics.
        if menu && has_probe {
            commands.entity(e).remove::<bevy::light::AtmosphereEnvironmentMapLight>();
        } else if !menu && !has_probe {
            commands.entity(e).insert(bevy::light::AtmosphereEnvironmentMapLight::default());
        }
        if two_cam == is_ui {
            if two_cam { commands.entity(e).remove::<IsDefaultUiCamera>(); } else { commands.entity(e).insert(IsDefaultUiCamera); }
        }
    }
    for (e, mut c, is_ui) in &mut ui {
        if c.is_active != two_cam {
            c.is_active = two_cam;
        }
        c.clear_color = ClearColorConfig::Default;
        if two_cam != is_ui {
            if two_cam { commands.entity(e).insert(IsDefaultUiCamera); } else { commands.entity(e).remove::<IsDefaultUiCamera>(); }
        }
    }
}

fn apply_render_scale(
    gfx: Res<GraphicsSettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cams: Query<(Entity, &mut RenderTarget), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    mut state: ResMut<ScaleState>,
    mut images: ResMut<Assets<Image>>,
    mut fsr: ResMut<Assets<FsrMaterial>>,
    mut commands: Commands,
) {
    if crate::native::enabled() {
        return; // the native renderer owns the world; no Bevy 3D target to upscale
    }
    let Ok(win) = windows.single() else { return };
    let phys = UVec2::new(win.physical_width().max(1), win.physical_height().max(1));
    if gfx.render_pct >= 100 {
        if state.image.is_some() {
            for (e, mut t) in &mut cams {
                *t = RenderTarget::default();
                commands.entity(e).remove::<ContrastAdaptiveSharpening>();
            }
            for e in state.extras.drain(..) {
                commands.entity(e).despawn();
            }
            state.image = None;
            state.size = UVec2::ZERO;
        }
        return;
    }
    let want = (phys.as_vec2() * gfx.render_pct as f32 / 100.0).as_uvec2().max(UVec2::splat(64));
    if state.image.is_some() && state.size == want {
        return;
    }
    for e in state.extras.drain(..) {
        commands.entity(e).despawn();
    }
    let handle = images.add(Image::new_target_texture(want.x, want.y, TextureFormat::Rgba8UnormSrgb, None));
    let use_fsr = fsr_enabled();
    // Sharpen more the further we upscale (FSR pairs EASU with RCAS ~0.2 stops).
    let sharp = (0.25 + (100.0 - gfx.render_pct as f32) / 100.0).clamp(0.2, 0.8);
    for (e, mut t) in &mut cams {
        *t = RenderTarget::Image(handle.clone().into());
        if use_fsr {
            commands.entity(e).insert(ContrastAdaptiveSharpening { enabled: true, sharpening_strength: sharp, denoise: false });
        }
    }
    // Window camera that only draws UI, plus the upscaled 3D image behind it.
    let node = Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    let quad = if use_fsr {
        commands.spawn((node, MaterialNode(fsr.add(FsrMaterial { source: handle.clone() })), GlobalZIndex(-1000), Pickable::IGNORE)).id()
    } else {
        commands.spawn((node, ImageNode::new(handle.clone()), GlobalZIndex(-1000), Pickable::IGNORE)).id()
    };
    state.extras = vec![quad];
    state.image = Some(handle);
    state.size = want;
}
