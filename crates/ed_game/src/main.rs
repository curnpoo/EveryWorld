//! EveryWorld: drop anywhere on the real Earth.

mod assets;
mod body;
mod building_materials;
mod camera;
mod capture;
mod cloud_half;
mod clouds;
mod config;
mod engine_audio;
mod soundscape;
mod settings;
mod boot_plume;
mod foliage;
mod gamepad;
mod ground_material;
mod ground_scatter;
mod hud;
mod impostors;
mod library;
mod loading;
mod lookdev;
mod globe;
mod headless;
mod mapview;
mod native;
mod near_chunks;
mod menu;
mod menu_music;
mod meshconv;
mod minimap;
mod waypoint;
mod net;
mod origin;
mod perf;
mod render_scale;
mod player;
mod probes;
mod probe_gi;
mod shadows;
mod far_shadows;
mod sky;
mod sonic_boom;
mod spawn;
mod streaming;
mod vehicles;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::window::WindowResolution;

fn main() {
    // gilrs has no SDL mapping for many controllers on Windows; provide the
    // standard Xbox mapping so the sticks/triggers work out of the box.
    if cfg!(target_os = "windows") {
        unsafe { std::env::set_var("SDL_GAMECONTROLLERCONFIG", "030000005e040000ea02000000000000,Xbox One Controller,a:b0,b:b1,x:b2,y:b3,back:b6,guide:b10,start:b7,leftstick:b8,rightstick:b9,leftshoulder:b4,rightshoulder:b5,dpup:h0.1,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,leftx:a0,lefty:a1,rightx:a2,righty:a3,lefttrigger:a4,righttrigger:a5,"); }
    }
    let cfg = config::Config::from_args();
    let title = format!("EveryWorld - {}", cfg.place_name);
    let mut resolution = WindowResolution::new(cfg.width, cfg.height);
    if let Some(scale) = cfg.scale {
        resolution = resolution.with_scale_factor_override(scale as f32);
    }
    let mut app = App::new();
    let mut plugins = DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title,
                    resolution,
                    // Mouse-look latency: default Fifo + 2 queued frames adds ~2 frames of input lag,
                    // which at low fps (~25) is >80 ms and makes the camera feel heavy/steppy.
                    present_mode: bevy::window::PresentMode::AutoNoVsync,
                    desired_maximum_frame_latency: std::num::NonZeroU32::new(1),
                    transparent: native::enabled(),
                    composite_alpha_mode: if native::enabled() { bevy::window::CompositeAlphaMode::PostMultiplied } else { default() },
                    ..default()
                }),
                ..default()
            })
            .set(bevy::log::LogPlugin { filter: "wgpu=error,naga=warn,bevy_render=warn".into(), ..default() });
    // Pipelined rendering draws frame N-1 while simulating N: more throughput, one more frame of input lag.
    // EW_NO_PIPELINE=1 trades some fps for snappier mouse look (A/B it at low fps).
    if std::env::var_os("EW_NO_PIPELINE").is_some() {
        plugins = plugins.disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>();
    }
    if cfg.headless {
        // No winit: no window, no Dock icon, no focus steal. The loop runs
        // uncapped; `headless::wait_for_gpu` paces it against the GPU.
        plugins = plugins
            .set(WindowPlugin { primary_window: None, exit_condition: bevy::window::ExitCondition::DontExit, ..default() })
            .disable::<bevy::winit::WinitPlugin>();
        app.add_plugins(bevy::app::ScheduleRunnerPlugin::run_loop(std::time::Duration::ZERO));
    }
    app.add_plugins(plugins)
    .add_plugins(bevy::pbr::experimental::meshlet::MeshletPlugin { cluster_buffer_slots: if cfg!(target_os = "windows") { 1 << 18 } else { 1 << 22 } })
    .add_plugins(PhysicsPlugins::default())
    .insert_resource(Gravity(Vec3::NEG_Y * 9.81))
    .insert_resource(cfg.clone())
    .insert_resource(bevy::light::DirectionalLightShadowMap { size: if cfg!(target_os = "windows") { 1024 } else { 2048 } })
    .add_plugins((origin::OriginPlugin, assets::AssetsPlugin, building_materials::BuildingMaterialsPlugin, sky::SkyPlugin, shadows::ShadowsPlugin, far_shadows::FarShadowsPlugin, camera::CameraPlugin, foliage::FoliagePlugin))
    .add_plugins((clouds::CloudsPlugin, cloud_half::CloudHalfPlugin, perf::PerfPlugin, render_scale::RenderScalePlugin, library::LibraryPlugin, native::NativePlugin, loading::LoadingPlugin))
    .add_systems(Update, headless::quit_after);
    if cfg.headless {
        app.add_plugins(headless::HeadlessPlugin);
    } else if cfg.lookdev.is_none() {
        // `EW_HOP` fast-travel harness, also usable for windowed visual checks.
        app.add_systems(Update, headless::scripted_hops);
    }
    if let Some(dir) = &cfg.lookdev {
        std::fs::create_dir_all(dir).expect("create lookdev dir");
        // render_scale::sync_cameras needs these; the menu plugin that owns them isn't loaded here.
        app.insert_state(menu::GameState::Playing).init_resource::<menu::GraphicsSettings>();
        app.insert_resource(lookdev::Lookdev { out_dir: dir.clone(), only: cfg.lookdev_only.clone() })
            .add_plugins((lookdev::LookdevPlugin, body::BodyPlugin));
    } else {
        app.add_plugins((streaming::StreamingPlugin, capture::CapturePlugin, hud::HudPlugin, minimap::MinimapPlugin, probe_gi::ProbeGiPlugin, spawn::SpawnPlugin, mapview::MapViewPlugin, globe::GlobePlugin, settings::SettingsPlugin, menu::MenuPlugin, gamepad::GamepadPlugin, net::NetPlugin, body::BodyPlugin, waypoint::WaypointPlugin));
        if !cfg.headless {
            app.add_plugins((soundscape::SoundscapePlugin, menu_music::MenuMusicPlugin));
        }
        if !cfg.fly {
            if cfg!(target_os = "windows") {
                app.add_plugins((player::PlayerPlugin, vehicles::VehiclesPlugin, engine_audio::EngineAudioPlugin, sonic_boom::SonicBoomPlugin));
            } else {
                app.add_plugins((player::PlayerPlugin, boot_plume::BootPlumePlugin, vehicles::VehiclesPlugin, engine_audio::EngineAudioPlugin, sonic_boom::SonicBoomPlugin));
            }
        }
    }
    app.run();
}
