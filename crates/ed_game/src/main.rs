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
mod foliage;
mod ground_material;
mod ground_scatter;
mod hud;
mod impostors;
mod library;
mod lookdev;
mod globe;
mod headless;
mod mapview;
mod near_chunks;
mod menu;
mod meshconv;
mod minimap;
mod origin;
mod perf;
mod render_scale;
mod player;
mod probe_gi;
mod sky;
mod sonic_boom;
mod spawn;
mod streaming;
mod vehicles;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::window::WindowResolution;

fn main() {
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
    .add_plugins(bevy::pbr::experimental::meshlet::MeshletPlugin { cluster_buffer_slots: 1 << 22 })
    .add_plugins(PhysicsPlugins::default())
    .insert_resource(Gravity(Vec3::NEG_Y * 9.81))
    .insert_resource(cfg.clone())
    .add_plugins((origin::OriginPlugin, assets::AssetsPlugin, building_materials::BuildingMaterialsPlugin, sky::SkyPlugin, camera::CameraPlugin, foliage::FoliagePlugin))
    .add_plugins((clouds::CloudsPlugin, cloud_half::CloudHalfPlugin, perf::PerfPlugin, render_scale::RenderScalePlugin, library::LibraryPlugin))
    .add_systems(Update, headless::quit_after);
    if cfg.headless {
        app.add_plugins(headless::HeadlessPlugin);
    }
    if let Some(dir) = &cfg.lookdev {
        std::fs::create_dir_all(dir).expect("create lookdev dir");
        app.insert_resource(lookdev::Lookdev { out_dir: dir.clone(), only: cfg.lookdev_only.clone() })
            .add_plugins((lookdev::LookdevPlugin, body::BodyPlugin));
    } else {
        app.add_plugins((streaming::StreamingPlugin, capture::CapturePlugin, hud::HudPlugin, minimap::MinimapPlugin, probe_gi::ProbeGiPlugin, spawn::SpawnPlugin, mapview::MapViewPlugin, globe::GlobePlugin, menu::MenuPlugin));
        if !cfg.fly {
            app.add_plugins((player::PlayerPlugin, body::BodyPlugin, vehicles::VehiclesPlugin, engine_audio::EngineAudioPlugin, sonic_boom::SonicBoomPlugin));
        }
    }
    app.run();
}
