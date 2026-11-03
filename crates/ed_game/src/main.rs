//! EarthDrop: drop anywhere on the real Earth.

mod assets;
mod body;
mod camera;
mod capture;
mod config;
mod hud;
mod lookdev;
mod meshconv;
mod origin;
mod player;
mod sky;
mod streaming;
mod vehicles;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::window::WindowResolution;

fn main() {
    let cfg = config::Config::from_args();
    let title = format!("EarthDrop - {}", cfg.place_name);
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title,
                    resolution: WindowResolution::new(cfg.width, cfg.height),
                    ..default()
                }),
                ..default()
            })
            .set(bevy::log::LogPlugin { filter: "wgpu=error,naga=warn,bevy_render=warn".into(), ..default() }),
    )
    .add_plugins(PhysicsPlugins::default())
    .insert_resource(Gravity(Vec3::NEG_Y * 9.81))
    .insert_resource(cfg.clone())
    .add_plugins((origin::OriginPlugin, assets::AssetsPlugin, sky::SkyPlugin, camera::CameraPlugin));
    if let Some(dir) = &cfg.lookdev {
        std::fs::create_dir_all(dir).expect("create lookdev dir");
        app.insert_resource(lookdev::Lookdev { out_dir: dir.clone(), only: cfg.lookdev_only.clone() })
            .add_plugins((lookdev::LookdevPlugin, body::BodyPlugin));
    } else {
        app.add_plugins((streaming::StreamingPlugin, capture::CapturePlugin, hud::HudPlugin));
        if !cfg.fly {
            app.add_plugins((player::PlayerPlugin, body::BodyPlugin, vehicles::VehiclesPlugin));
        }
    }
    app.run();
}
