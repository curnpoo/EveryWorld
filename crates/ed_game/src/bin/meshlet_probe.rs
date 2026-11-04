//! Feasibility probe for Bevy's meshlet (virtual geometry) pipeline on this GPU.
//!
//!   cargo run --release --features meshlet --bin meshlet_probe -- [--standard] [--grid 60] [--ico 40]
//!
//! Spawns grid*grid dense spheres (default 60x60 x ~33k tris = ~120M tris) with either the
//! meshlet pipeline (default) or the regular renderer (--standard), prints fps once a second
//! and exits after ~15 s with the average. Not part of the game.

use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    pbr::experimental::meshlet::{MeshletMesh, MeshletMesh3d, MeshletPlugin, MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR},
    prelude::*,
};

fn arg(name: &str, default: u32) -> u32 {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[derive(Resource)]
struct Cfg { standard: bool, grid: u32, ico: u32 }

#[derive(Resource, Default)]
struct Samples(Vec<f64>);

fn main() {
    let standard = std::env::args().any(|a| a == "--standard");
    let cfg = Cfg { standard, grid: arg("--grid", 60), ico: arg("--ico", 40) };
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, FrameTimeDiagnosticsPlugin::default()));
    app.add_plugins(MeshletPlugin { cluster_buffer_slots: 1 << 22 });
    app.insert_resource(cfg).init_resource::<Samples>().add_systems(Startup, setup).add_systems(Update, report).run();
}

fn setup(mut commands: Commands, cfg: Res<Cfg>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut mm: ResMut<Assets<MeshletMesh>>) {
    commands.spawn((Camera3d::default(), Msaa::Off, Transform::from_xyz(0.0, 25.0, 80.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y)));
    commands.spawn((DirectionalLight { shadow_maps_enabled: false, illuminance: 20_000.0, ..default() }, Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.4, 0.0))));
    let mat = mats.add(StandardMaterial { base_color: Color::srgb(0.3, 0.6, 0.3), perceptual_roughness: 0.8, ..default() });
    let sphere = Sphere::new(1.0).mesh().ico(cfg.ico).unwrap();
    let tris = sphere.indices().map(|i| i.len() / 3).unwrap_or(0);
    println!("mode={} grid={}x{} tris/instance={} total={:.1}M", if cfg.standard { "standard" } else { "meshlet" }, cfg.grid, cfg.grid, tris, (tris as f64 * (cfg.grid * cfg.grid) as f64) / 1e6);
    let t0 = std::time::Instant::now();
    let mlet = if cfg.standard { None } else {
        let m = MeshletMesh::from_mesh(&sphere, MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR).expect("meshlet conversion");
        println!("meshlet conversion {:.2}s", t0.elapsed().as_secs_f32());
        Some(mm.add(m))
    };
    let std_mesh = meshes.add(sphere);
    let n = cfg.grid as i32;
    for x in 0..n { for z in 0..n {
        let t = Transform::from_xyz((x - n / 2) as f32 * 2.5, 0.0, (z - n / 2) as f32 * 2.5);
        match &mlet {
            Some(h) => { commands.spawn((MeshletMesh3d(h.clone()), MeshMaterial3d(mat.clone()), t)); }
            None => { commands.spawn((Mesh3d(std_mesh.clone()), MeshMaterial3d(mat.clone()), t)); }
        }
    } }
}

fn report(time: Res<Time<Real>>, diag: Res<DiagnosticsStore>, mut s: ResMut<Samples>, mut last: Local<f32>, mut exit: MessageWriter<AppExit>) {
    let t = time.elapsed_secs();
    if t - *last < 1.0 { return; }
    *last = t;
    if let Some(fps) = diag.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()) {
        println!("t={t:.0}s fps={fps:.1}");
        if t > 4.0 { s.0.push(fps); }
    }
    if t > 15.0 {
        let avg = s.0.iter().sum::<f64>() / s.0.len().max(1) as f64;
        println!("AVG fps={avg:.1}");
        exit.write(AppExit::Success);
    }
}
