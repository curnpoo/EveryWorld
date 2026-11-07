//! Ghost multiplayer client.

use crate::config::Config;
use crate::origin::WorldOrigin;
use crate::player::Player;
use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender};
use ed_net::{decode_s2c, encode, C2S, S2C, Vec3d, PROTOCOL_VERSION};
use glam::DVec3;
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Resource, Default)]
pub struct RemotePlayers {
    pub players: HashMap<u64, RemotePlayer>,
}

#[derive(Clone, Debug)]
pub struct RemotePlayer {
    pub id: u64,
    pub name: String,
    pub pos: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub boot: u8,
    pub throttle: f32,
    pub wingsuit: bool,
    pub last: f64,
    // interpolation buffer
    pub prev_pos: [f64; 3],
    pub prev_yaw: f32,
    pub prev_pitch: f32,
    pub prev_boot: u8,
    pub prev_throttle: f32,
    pub prev_wingsuit: bool,
    pub prev_t: f64,
}

#[derive(Resource)]
pub struct NetClient {
    pub status: NetStatus,
    pub server: Option<SocketAddr>,
    pub id: Option<u64>,
    pub name: String,
    pub target: String,
    pub cmd_tx: Sender<NetCmd>,
    pub evt_rx: Receiver<NetEvt>,
    pub players: Arc<Mutex<HashMap<u64, RemotePlayer>>>,
    pub spawned_near: bool,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum NetStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
}

pub enum NetCmd {
    Connect { addr: SocketAddr, name: String },
    Disconnect,
    State { pos: [f64; 3], yaw: f32, pitch: f32, boot: u8, throttle: f32, wingsuit: bool, t: f64 },
}

pub enum NetEvt {
    Connected { id: u64, players: Vec<(u64, String)> },
    Join { id: u64, name: String },
    State { id: u64, name: String, pos: [f64; 3], yaw: f32, pitch: f32, boot: u8, throttle: f32, wingsuit: bool, t: f64 },
    Leave { id: u64 },
    Disconnected,
}

#[derive(Resource, Default)]
pub struct RemoteAvatarEntities {
    pub map: HashMap<u64, Entity>,
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
        let (evt_tx, evt_rx) = crossbeam_channel::unbounded();
        let players = Arc::new(Mutex::new(HashMap::new()));
        let players_bg = players.clone();
        thread::spawn(move || net_thread(cmd_rx, evt_tx, players_bg));

        let cfg = app.world().get_resource::<Config>().cloned();
        let target = cfg.as_ref().and_then(|c| c.server.clone()).unwrap_or_else(|| "127.0.0.1:7777".to_string());
        let name = cfg.as_ref().and_then(|c| c.player_name.clone()).unwrap_or_else(|| "player".to_string());

        app.insert_resource(NetClient {
            status: NetStatus::Disconnected,
            server: None,
            id: None,
            name: name.clone(),
            target,
            cmd_tx,
            evt_rx,
            players: players.clone(),
            spawned_near: false,
        })
        .insert_resource(RemotePlayers::default())
        .insert_resource(RemoteAvatarEntities::default())
        .insert_resource(RemoteNametagEntities::default())
        .insert_resource(SendAccum::default())
        .insert_resource(RemotePhase::default())
        .add_systems(Update, (net_drain_events, net_send_state, net_spawn_avatars, net_update_avatars, net_update_nametags, net_update_status_text, net_menu_actions).chain());
    }
}

fn net_thread(cmd_rx: Receiver<NetCmd>, evt_tx: Sender<NetEvt>, players: Arc<Mutex<HashMap<u64, RemotePlayer>>>) {
    let mut socket: Option<UdpSocket> = None;
    let mut server: Option<SocketAddr> = None;
    let mut buf = [0u8; 4096];
    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                NetCmd::Connect { addr, name: _ } => {
                    let s = UdpSocket::bind("0.0.0.0:0").ok();
                    if let Some(s) = &s {
                        s.set_nonblocking(true).ok();
                        let hello = encode(&C2S::Hello { name: "player".to_string(), protocol: PROTOCOL_VERSION });
                        let _ = s.send_to(&hello, addr);
                    }
                    socket = s;
                    server = Some(addr);
                }
                NetCmd::Disconnect => {
                    if let (Some(s), Some(a)) = (&socket, server) {
                        let _ = s.send_to(&encode(&C2S::Bye), a);
                    }
                    socket = None;
                    server = None;
                    let _ = evt_tx.send(NetEvt::Disconnected);
                    players.lock().unwrap().clear();
                }
                NetCmd::State { pos, yaw, pitch, boot, throttle, wingsuit, t } => {
                    if let (Some(s), Some(a)) = (&socket, server) {
                        let msg = encode(&C2S::State { pos: Vec3d::from(pos), yaw, pitch, boot, throttle, wingsuit, t });
                        let _ = s.send_to(&msg, a);
                    }
                }
            }
        }
        if let Some(s) = &socket {
            loop {
                match s.recv_from(&mut buf) {
                    Ok((n, _)) => {
                        if let Some(msg) = decode_s2c(&buf[..n]) {
                            match msg {
                                S2C::Welcome { id, players: roster } => {
                                    let _ = evt_tx.send(NetEvt::Connected { id, players: roster.into_iter().map(|p| (p.id, p.name)).collect() });
                                }
                                S2C::Join { id, name } => { let _ = evt_tx.send(NetEvt::Join { id, name }); }
                                S2C::State { id, name, pos, yaw, pitch, boot, throttle, wingsuit, t } => {
                                    let mut p = players.lock().unwrap();
                                    let prev = p.get(&id).cloned();
                                    let rp = RemotePlayer {
                                        id, name: name.clone(), pos: pos.into(), yaw, pitch, boot, throttle, wingsuit, last: t,
                                        prev_pos: prev.as_ref().map(|r| r.pos).unwrap_or(pos.into()),
                                        prev_yaw: prev.as_ref().map(|r| r.yaw).unwrap_or(yaw),
                                        prev_pitch: prev.as_ref().map(|r| r.pitch).unwrap_or(pitch),
                                        prev_boot: prev.as_ref().map(|r| r.boot).unwrap_or(boot),
                                        prev_throttle: prev.as_ref().map(|r| r.throttle).unwrap_or(throttle),
                                        prev_wingsuit: prev.as_ref().map(|r| r.wingsuit).unwrap_or(wingsuit),
                                        prev_t: prev.as_ref().map(|r| r.last).unwrap_or(t),
                                    };
                                    p.insert(id, rp);
                                    let _ = evt_tx.send(NetEvt::State { id, name, pos: pos.into(), yaw, pitch, boot, throttle, wingsuit, t });
                                }
                                S2C::Leave { id } => {
                                    players.lock().unwrap().remove(&id);
                                    let _ = evt_tx.send(NetEvt::Leave { id });
                                }
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn net_drain_events(
    mut client: ResMut<NetClient>,
    mut remote: ResMut<RemotePlayers>,
) {
    let mut roster: Vec<(u64, String)> = vec![];
    let mut joins: Vec<(u64, String)> = vec![];
    let mut leaves: Vec<u64> = vec![];
    let mut states: Vec<(u64, String, [f64; 3], f32, f32, u8, f32, bool, f64)> = vec![];
    let mut connected_now = false;
    let mut disconnected = false;

    while let Ok(evt) = client.evt_rx.try_recv() {
        match evt {
            NetEvt::Connected { id, players } => {
                client.id = Some(id);
                client.status = NetStatus::Connected;
                client.spawned_near = false;
                roster = players;
                connected_now = true;
            }
            NetEvt::Join { id, name } => joins.push((id, name)),
            NetEvt::State { id, name, pos, yaw, pitch, boot, throttle, wingsuit, t } => states.push((id, name, pos, yaw, pitch, boot, throttle, wingsuit, t)),
            NetEvt::Leave { id } => leaves.push(id),
            NetEvt::Disconnected => { disconnected = true; }
        }
    }

    if connected_now {
        for (id, name) in roster {
            remote.players.insert(id, RemotePlayer { id, name, pos: [0.0; 3], yaw: 0.0, pitch: 0.0, boot: 0, throttle: 0.0, wingsuit: false, last: 0.0, prev_pos: [0.0;3], prev_yaw: 0.0, prev_pitch: 0.0, prev_boot: 0, prev_throttle: 0.0, prev_wingsuit: false, prev_t: 0.0 });
        }
    }
    for (id, name) in joins {
        remote.players.insert(id, RemotePlayer { id, name, pos: [0.0; 3], yaw: 0.0, pitch: 0.0, boot: 0, throttle: 0.0, wingsuit: false, last: 0.0, prev_pos: [0.0;3], prev_yaw: 0.0, prev_pitch: 0.0, prev_boot: 0, prev_throttle: 0.0, prev_wingsuit: false, prev_t: 0.0 });
    }
    for (id, name, pos, yaw, pitch, boot, throttle, wingsuit, t) in states {
        if let Some(rp) = remote.players.get_mut(&id) {
            rp.prev_pos = rp.pos; rp.prev_yaw = rp.yaw; rp.prev_pitch = rp.pitch; rp.prev_boot = rp.boot; rp.prev_throttle = rp.throttle; rp.prev_wingsuit = rp.wingsuit; rp.prev_t = rp.last;
            rp.name = name; rp.pos = pos; rp.yaw = yaw; rp.pitch = pitch; rp.boot = boot; rp.throttle = throttle; rp.wingsuit = wingsuit; rp.last = t;
        } else {
            remote.players.insert(id, RemotePlayer { id, name, pos, yaw, pitch, boot, throttle, wingsuit, last: t, prev_pos: pos, prev_yaw: yaw, prev_pitch: pitch, prev_boot: boot, prev_throttle: throttle, prev_wingsuit: wingsuit, prev_t: t });
        }
    }
    for id in leaves {
        remote.players.remove(&id);
    }
    if disconnected {
        client.status = NetStatus::Disconnected;
        client.id = None;
        client.server = None;
        remote.players.clear();
    }
}

#[derive(Resource, Default)]
struct SendAccum(f32);

fn net_send_state(
    client: Res<NetClient>,
    origin: Res<WorldOrigin>,
    time: Res<Time>,
    mut acc: ResMut<SendAccum>,
    q: Query<(&Transform, &Player), With<Player>>,
) {
    if client.status != NetStatus::Connected {
        return;
    }
    acc.0 += time.delta_secs();
    if acc.0 < 1.0 / 60.0 {
        return;
    }
    acc.0 = 0.0;
    if let Ok((t, p)) = q.single() {
        let render = t.translation.as_dvec3();
        let ecef = origin.frame.render_to_ecef().transform_point3(render);
        let _ = client.cmd_tx.send(NetCmd::State {
            pos: [ecef.x, ecef.y, ecef.z],
            yaw: p.yaw,
            pitch: p.cam_pitch,
            boot: p.mode as u8,
            throttle: p.throttle,
            wingsuit: p.wingsuit_on,
            t: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0),
        });
    }
}

#[derive(Component)]
pub struct RemoteAvatar {
    pub id: u64,
}

#[derive(Resource, Default)]
struct RemotePhase {
    map: HashMap<u64, f32>,
}

// Robot part tags for remote avatars (pose computed on the viewer).
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum RJ { Pelvis, Chest, Neck, Head, HipL, HipR, KneeL, KneeR, AnkleL, AnkleR, ShoulderL, ShoulderR, ElbowL, ElbowR, WristL, WristR }
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum RF { Pelvis, Chest, Head, FootL, FootR, HandL, HandR }
#[derive(Component, Clone, Debug)]
enum RSpec {
    Limb { a: RJ, b: RJ, r: f32 },
    Ball { j: RJ, r: f32 },
    Rigid { f: RF, off: Vec3, rot: Quat, scale: Vec3 },
    Eye { off: Vec3 },
    Jet { foot: usize, inner: bool },
    Canopy,
}
#[derive(Component)]
struct RPart { id: u64, spec: RSpec }
#[derive(Component)]
struct RWing { id: u64, k: usize }

fn r_limb(a: Vec3, b: Vec3, r: f32) -> Transform {
    let d = b - a;
    let len = d.length().max(0.001);
    Transform { translation: (a+b)*0.5, rotation: Quat::from_rotation_arc(Vec3::Y, d/len), scale: Vec3::new(r*2.0, len, r*2.0) }
}
fn r_wing_blank() -> bevy::mesh::Mesh {
    use bevy::mesh::{Indices, PrimitiveTopology};
    let mut idx = Vec::new();
    for k in 0..7u32 { idx.extend_from_slice(&[7,k,(k+1)%7]); }
    bevy::mesh::Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default())
        .with_inserted_attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION, vec![[0.0f32;3];8])
        .with_inserted_attribute(bevy::mesh::Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32,1.0,0.0];8])
        .with_inserted_attribute(bevy::mesh::Mesh::ATTRIBUTE_UV_0, vec![[0.0f32;2];8])
        .with_inserted_indices(Indices::U32(idx))
}

#[derive(Component)]
pub struct RemoteNametag {
    pub id: u64,
}

#[derive(Resource, Default)]
pub struct RemoteNametagEntities {
    pub map: HashMap<u64, Entity>,
}

pub fn net_spawn_avatars(
    mut commands: Commands,
    mut meshes: ResMut<Assets<bevy::mesh::Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    remote: Res<RemotePlayers>,
    mut ents: ResMut<RemoteAvatarEntities>,
    mut nametag_entities: ResMut<RemoteNametagEntities>,
) {
    for (id, rp) in remote.players.iter() {
        if ents.map.contains_key(id) {
            continue;
        }
        // materials (viewer-local)
        let mk = |mats: &mut Assets<StandardMaterial>, rgb: (f32,f32,f32), rough: f32, metal: f32| mats.add(StandardMaterial { base_color: Color::srgb(rgb.0,rgb.1,rgb.2), perceptual_roughness: rough, metallic: metal, ..default() });
        let shell = mk(&mut materials, (0.92,0.89,0.82), 0.4, 0.0);
        let shell_dark = mk(&mut materials, (0.78,0.74,0.66), 0.45, 0.0);
        let joint = mk(&mut materials, (0.22,0.23,0.26), 0.45, 0.6);
        let jacket = mk(&mut materials, (0.96,0.42,0.09), 0.85, 0.0);
        let jacket_dark = mk(&mut materials, (0.72,0.28,0.05), 0.85, 0.0);
        let shoe = mk(&mut materials, (0.24,0.25,0.28), 0.7, 0.0);
        let sole = mk(&mut materials, (0.95,0.95,0.93), 0.6, 0.0);
        let brass = mk(&mut materials, (0.85,0.66,0.32), 0.3, 0.9);
        let screen = materials.add(StandardMaterial { base_color: Color::srgb(0.03,0.06,0.07), perceptual_roughness: 0.06, reflectance: 0.9, ..default() });
        let eye = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(0.6,3.0,3.2)*2000.0, ..default() });
        let flame_outer = materials.add(StandardMaterial { base_color: Color::srgb(1.0,0.35,0.05), emissive: LinearRgba::rgb(4.5,1.4,0.15)*3500.0, unlit: true, ..default() });
        let flame_core = materials.add(StandardMaterial { base_color: Color::srgb(0.85,0.95,1.0), emissive: LinearRgba::rgb(2.5,4.0,6.0)*5500.0, unlit: true, ..default() });
        let canopy = materials.add(StandardMaterial { base_color: Color::srgb(0.96,0.42,0.09), perceptual_roughness: 0.9, double_sided: true, cull_mode: None, ..default() });
        let wing_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.10,0.16,0.34), perceptual_roughness: 0.6, double_sided: true, cull_mode: None, ..default() });

        let cyl = meshes.add(Cylinder::new(0.5,1.0).mesh().resolution(20));
        let ball = meshes.add(Sphere::new(0.5).mesh().uv(20,12));
        let cube = meshes.add(Cuboid::new(1.0,1.0,1.0));
        let dome = meshes.add(Sphere::new(1.0).mesh().uv(24,8));
        let cone = meshes.add(Cone::new(0.5,1.0).mesh().resolution(16));

        let q0 = Quat::IDENTITY;
        let qx = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let qz = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let v = Vec3::new;
        use RJ::*; use RF::*;
        let parts: Vec<(RSpec, &bevy::asset::Handle<bevy::mesh::Mesh>, &bevy::asset::Handle<StandardMaterial>)> = vec![
            (RSpec::Limb{a:HipL,b:KneeL,r:0.075}, &cyl, &shell),
            (RSpec::Limb{a:HipR,b:KneeR,r:0.075}, &cyl, &shell),
            (RSpec::Limb{a:KneeL,b:AnkleL,r:0.065}, &cyl, &shell),
            (RSpec::Limb{a:KneeR,b:AnkleR,r:0.065}, &cyl, &shell),
            (RSpec::Ball{j:HipL,r:0.1}, &ball, &joint),
            (RSpec::Ball{j:HipR,r:0.1}, &ball, &joint),
            (RSpec::Ball{j:KneeL,r:0.088}, &ball, &joint),
            (RSpec::Ball{j:KneeR,r:0.088}, &ball, &joint),
            (RSpec::Ball{j:AnkleL,r:0.07}, &ball, &joint),
            (RSpec::Ball{j:AnkleR,r:0.07}, &ball, &joint),
            (RSpec::Rigid{f:FootL, off:v(0.0,-0.025,-0.05), rot:q0, scale:v(0.13,0.08,0.27)}, &cube, &shoe),
            (RSpec::Rigid{f:FootR, off:v(0.0,-0.025,-0.05), rot:q0, scale:v(0.13,0.08,0.27)}, &cube, &shoe),
            (RSpec::Rigid{f:FootL, off:v(0.0,-0.075,-0.05), rot:q0, scale:v(0.14,0.03,0.29)}, &cube, &sole),
            (RSpec::Rigid{f:FootR, off:v(0.0,-0.075,-0.05), rot:q0, scale:v(0.14,0.03,0.29)}, &cube, &sole),
            (RSpec::Rigid{f:FootL, off:v(0.0,-0.035,-0.17), rot:qz, scale:v(0.08,0.13,0.08)}, &cyl, &shoe),
            (RSpec::Rigid{f:FootR, off:v(0.0,-0.035,-0.17), rot:qz, scale:v(0.08,0.13,0.08)}, &cyl, &shoe),
            (RSpec::Rigid{f:FootL, off:v(0.0,-0.095,-0.03), rot:q0, scale:v(0.09,0.025,0.09)}, &cyl, &joint),
            (RSpec::Rigid{f:FootR, off:v(0.0,-0.095,-0.03), rot:q0, scale:v(0.09,0.025,0.09)}, &cyl, &joint),
            (RSpec::Jet{foot:0,inner:false}, &cone, &flame_outer),
            (RSpec::Jet{foot:0,inner:true}, &cone, &flame_core),
            (RSpec::Jet{foot:1,inner:false}, &cone, &flame_outer),
            (RSpec::Jet{foot:1,inner:true}, &cone, &flame_core),
            (RSpec::Rigid{f:Pelvis, off:v(0.0,0.0,0.0), rot:q0, scale:v(0.34,0.16,0.22)}, &cube, &joint),
            (RSpec::Limb{a:Pelvis,b:Chest,r:0.08}, &cyl, &joint),
            (RSpec::Rigid{f:Chest, off:v(0.0,-0.2,0.0), rot:q0, scale:v(0.46,0.4,0.28)}, &cube, &jacket),
            (RSpec::Rigid{f:Chest, off:v(0.0,-0.41,0.0), rot:q0, scale:v(0.44,0.06,0.26)}, &cube, &jacket_dark),
            (RSpec::Rigid{f:Chest, off:v(0.0,0.02,0.0), rot:q0, scale:v(0.3,0.07,0.22)}, &cube, &jacket_dark),
            (RSpec::Rigid{f:Chest, off:v(0.0,-0.2,-0.142), rot:q0, scale:v(0.025,0.36,0.01)}, &cube, &brass),
            (RSpec::Limb{a:Chest,b:Neck,r:0.055}, &cyl, &joint),
            (RSpec::Limb{a:ShoulderL,b:ElbowL,r:0.07}, &cyl, &jacket),
            (RSpec::Limb{a:ShoulderR,b:ElbowR,r:0.07}, &cyl, &jacket),
            (RSpec::Limb{a:ElbowL,b:WristL,r:0.05}, &cyl, &shell),
            (RSpec::Limb{a:ElbowR,b:WristR,r:0.05}, &cyl, &shell),
            (RSpec::Ball{j:ShoulderL,r:0.11}, &ball, &jacket),
            (RSpec::Ball{j:ShoulderR,r:0.11}, &ball, &jacket),
            (RSpec::Ball{j:ElbowL,r:0.07}, &ball, &joint),
            (RSpec::Ball{j:ElbowR,r:0.07}, &ball, &joint),
            (RSpec::Ball{j:WristL,r:0.05}, &ball, &joint),
            (RSpec::Ball{j:WristR,r:0.05}, &ball, &joint),
            (RSpec::Rigid{f:HandL, off:v(0.0,-0.06,0.0), rot:q0, scale:v(0.07,0.11,0.09)}, &cube, &shell),
            (RSpec::Rigid{f:HandR, off:v(0.0,-0.06,0.0), rot:q0, scale:v(0.07,0.11,0.09)}, &cube, &shell),
            (RSpec::Rigid{f:Head, off:v(0.0,0.0,0.0), rot:q0, scale:v(0.42,0.34,0.3)}, &cube, &shell),
            (RSpec::Rigid{f:Head, off:v(0.0,0.0,-0.152), rot:q0, scale:v(0.38,0.3,0.01)}, &cube, &shell_dark),
            (RSpec::Rigid{f:Head, off:v(-0.03,0.005,-0.155), rot:q0, scale:v(0.29,0.24,0.05)}, &ball, &screen),
            (RSpec::Rigid{f:Head, off:v(0.0,-0.01,0.2), rot:q0, scale:v(0.3,0.24,0.14)}, &cube, &shell_dark),
            (RSpec::Rigid{f:Head, off:v(0.155,0.06,-0.155), rot:qx, scale:v(0.045,0.03,0.045)}, &cyl, &joint),
            (RSpec::Rigid{f:Head, off:v(0.09,0.27,0.0), rot:Quat::from_rotation_z(-0.35), scale:v(0.012,0.22,0.012)}, &cyl, &joint),
            (RSpec::Eye{off:v(-0.085,0.02,-0.18)}, &ball, &eye),
            (RSpec::Eye{off:v(0.035,0.02,-0.18)}, &ball, &eye),
            (RSpec::Canopy, &dome, &canopy),
        ];
        let entity = commands.spawn((
            Transform::default(),
            RemoteAvatar { id: *id },
            crate::origin::Rebased,
            Name::new(format!("remote_{}", rp.name)),
        )).with_children(|p| {
            for (spec, mesh, m) in &parts {
                p.spawn((RPart { id: *id, spec: spec.clone() }, Mesh3d(mesh.clone()), MeshMaterial3d(m.clone()), Transform::default(), Visibility::Visible));
            }
            for k in 0..3 {
                p.spawn((RWing { id: *id, k }, Mesh3d(meshes.add(r_wing_blank())), MeshMaterial3d(wing_mat.clone()), Transform::default(), Visibility::Hidden, bevy::camera::visibility::NoFrustumCulling));
            }
        }).id();
        // screen-space nametag with box
        let nametag = commands.spawn((
            RemoteNametag { id: *id },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-1000.0),
                top: Val::Px(-1000.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
            BorderColor::all(Color::srgb(1.0, 0.9, 0.2)),
            ZIndex(20),
        )).with_children(|p| {
            p.spawn((Text::new(rp.name.clone()), TextFont { font_size: bevy::text::FontSize::Px(18.0), ..default() }, TextColor(Color::srgb(1.0, 0.9, 0.2))));
        }).id();
        ents.map.insert(*id, entity);
        nametag_entities.map.insert(*id, nametag);
    }
    let to_remove: Vec<u64> = ents.map.keys().copied().filter(|id| !remote.players.contains_key(id)).collect();
    for id in to_remove {
        if let Some(e) = ents.map.remove(&id) {
            commands.entity(e).despawn();
        }
        if let Some(e) = nametag_entities.map.remove(&id) {
            commands.entity(e).despawn();
        }
    }
}

pub fn net_menu_actions(
    q: Query<(&Interaction, &crate::menu::MenuAction), Changed<Interaction>>,
    mut net: ResMut<NetClient>,
) {
    for (i, a) in q.iter() {
        if *i != Interaction::Pressed {
            continue;
        }
        match a {
            crate::menu::MenuAction::ConnectServer => {
                if let Ok(addr) = net.target.parse() {
                    net.cmd_tx.send(NetCmd::Connect { addr, name: net.name.clone() }).ok();
                    net.status = NetStatus::Connecting;
                }
            }
            crate::menu::MenuAction::DisconnectServer => {
                net.cmd_tx.send(NetCmd::Disconnect).ok();
                net.status = NetStatus::Disconnected;
            }
            _ => {}
        }
    }
}

fn net_update_nametags(
    remote: Res<RemotePlayers>,
    camera: Query<(&GlobalTransform, &Camera), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    avatars: Query<(&Transform, &RemoteAvatar)>,
    mut q: Query<(&mut Node, &RemoteNametag, &mut Visibility)>,
) {
    let cam = camera.iter().next();
    let pos_by_id: HashMap<u64, Vec3> = avatars.iter().map(|(t, a)| (a.id, t.translation)).collect();
    for (mut node, nt, mut vis) in q.iter_mut() {
        if !remote.players.contains_key(&nt.id) {
            *vis = Visibility::Hidden;
            continue;
        }
        let Some((cam_t, cam_c)) = cam else {
            *vis = Visibility::Hidden;
            continue;
        };
        let Some(world) = pos_by_id.get(&nt.id).copied() else {
            *vis = Visibility::Hidden;
            continue;
        };
        // distance check (100 km)
        let dist = (world - cam_t.translation()).length();
        if dist > 100_000.0 {
            *vis = Visibility::Hidden;
            continue;
        }
        if let Ok(screen) = cam_c.world_to_viewport(cam_t, world) {
            node.left = Val::Px(screen.x - 40.0);
            node.top = Val::Px(screen.y - 60.0);
            *vis = Visibility::Visible;
        } else {
            *vis = Visibility::Hidden;
        }
    }
}

fn net_update_status_text(
    client: Res<NetClient>,
    mut q: Query<&mut Text, With<crate::menu::NetStatusText>>,
) {
    let s = match client.status {
        NetStatus::Disconnected => format!("Disconnected (target {})", client.target),
        NetStatus::Connecting => format!("Connecting to {}...", client.target),
        NetStatus::Connected => format!("Connected as {} (id {:?})", client.name, client.id),
    };
    for mut t in q.iter_mut() {
        t.0 = s.clone();
    }
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let mut d = (b - a) % std::f32::consts::TAU;
    if d > std::f32::consts::PI { d -= std::f32::consts::TAU; }
    if d < -std::f32::consts::PI { d += std::f32::consts::TAU; }
    a + d * t
}

fn net_update_avatars(
    origin: Res<WorldOrigin>,
    remote: Res<RemotePlayers>,
    time: Res<Time>,
    mut phase: ResMut<RemotePhase>,
    mut q: Query<(&mut Transform, &RemoteAvatar)>,
    mut parts: Query<(&RPart, &mut Transform, &mut Visibility)>,
    mut wings: Query<(&RWing, &Mesh3d, &mut Visibility), Without<RPart>>,
    mut meshes: ResMut<Assets<bevy::mesh::Mesh>>,
) {
    let dt = time.delta_secs().clamp(1e-4, 0.05);
    // Collect avatar transforms by id
    let mut avatars: HashMap<u64, (Vec3, Quat, f32, u8, f32, bool, f32)> = HashMap::new(); // pos, rot, pitch, boot, throttle, wingsuit, speed
    for (mut t, av) in q.iter_mut() {
        let Some(rp) = remote.players.get(&av.id) else { continue };
        // interpolation: render at curr_t - 0.1s
        let span = (rp.last - rp.prev_t).max(1e-4);
        let alpha = ((rp.last - 0.1) - rp.prev_t) / span;
        let alpha = alpha.clamp(0.0, 1.0);
        let pos = [
            rp.prev_pos[0] + (rp.pos[0] - rp.prev_pos[0]) * alpha as f64,
            rp.prev_pos[1] + (rp.pos[1] - rp.prev_pos[1]) * alpha as f64,
            rp.prev_pos[2] + (rp.pos[2] - rp.prev_pos[2]) * alpha as f64,
        ];
        let yaw = lerp_angle(rp.prev_yaw, rp.yaw, alpha);
        let pitch = rp.prev_pitch + (rp.pitch - rp.prev_pitch) * alpha;
        let boot = if alpha > 0.5 { rp.boot } else { rp.prev_boot };
        let throttle = rp.prev_throttle + (rp.throttle - rp.prev_throttle) * alpha;
        let wingsuit = if alpha > 0.5 { rp.wingsuit } else { rp.prev_wingsuit };
        let ecef = DVec3::new(pos[0], pos[1], pos[2]);
        let render = origin.frame.ecef_to_render().transform_point3(ecef).as_vec3();
        // speed from span
        let delta = (rp.pos[0]-rp.prev_pos[0]).powi(2)+(rp.pos[1]-rp.prev_pos[1]).powi(2)+(rp.pos[2]-rp.prev_pos[2]).powi(2);
        let speed = (delta.sqrt() / span.max(1e-4)) as f32;
        t.translation = render;
        t.rotation = Quat::from_rotation_y(yaw);
        avatars.insert(av.id, (render, t.rotation, pitch, boot, throttle, wingsuit, speed));
        // phase for walk swing
        let ph = phase.map.entry(av.id).or_insert(0.0);
        *ph += speed * dt * 2.2;
    }
    // Animate parts per avatar: need joints in world space
    for (part, mut tf, mut vis) in parts.iter_mut() {
        let Some((root_pos, root_rot, pitch, boot, throttle, wingsuit, speed)) = avatars.get(&part.id).copied() else { *vis = Visibility::Hidden; continue };
        let _ = (pitch, wingsuit);
        let base = root_rot * Quat::from_rotation_x(pitch * 0.6);
        let up = Vec3::Y;
        let ph = phase.map.get(&part.id).copied().unwrap_or(0.0);
        // Local joints
        let pelvis = Vec3::ZERO;
        let chest = Vec3::new(0.0, 0.42, 0.0);
        let neck = Vec3::new(0.0, 0.65, 0.0);
        let head = Vec3::new(0.0, 0.85, 0.0);
        let hip_l = Vec3::new(-0.13, -0.04, 0.0);
        let hip_r = Vec3::new(0.13, -0.04, 0.0);
        // mode: 0 Walk,1 Fall,2 Rocket,3 Skydive,4 Parachute
        let (ankle_l, ankle_r, elbow_l, elbow_r, wrist_l, wrist_r) = match boot {
            0 => { // Walk
                let s = (ph).sin() * 0.28 * (speed.min(6.0)/4.0).clamp(0.0,1.0);
                let s2 = (ph + std::f32::consts::PI).sin() * 0.28 * (speed.min(6.0)/4.0).clamp(0.0,1.0);
                (hip_l + Vec3::new(0.0,-0.84,s), hip_r + Vec3::new(0.0,-0.84,s2),
                 Vec3::new(-0.35,-0.05,-s*0.5), Vec3::new(0.35,-0.05,-s2*0.5),
                 Vec3::new(-0.38,-0.30,s*0.5), Vec3::new(0.38,-0.30,s2*0.5))
            }
            2 => { // Rocket
                (hip_l+Vec3::new(0.0,-0.92,0.02), hip_r+Vec3::new(0.0,-0.92,0.02),
                 Vec3::new(-0.38,0.05,0.0), Vec3::new(0.38,0.05,0.0),
                 Vec3::new(-0.42,-0.10,0.0), Vec3::new(0.42,-0.10,0.0))
            }
            3 => { // Skydive
                (hip_l+Vec3::new(-0.1,-0.75,0.15), hip_r+Vec3::new(0.1,-0.75,0.15),
                 Vec3::new(-0.57,0.18,0.0), Vec3::new(0.57,0.18,0.0),
                 Vec3::new(-0.85,0.18,0.0), Vec3::new(0.85,0.18,0.0))
            }
            4 => { // Parachute
                (hip_l+Vec3::new(0.0,-0.86,0.05), hip_r+Vec3::new(0.0,-0.86,0.05),
                 Vec3::new(-0.35,0.45,0.0), Vec3::new(0.35,0.45,0.0),
                 Vec3::new(-0.30,0.70,0.0), Vec3::new(0.30,0.70,0.0))
            }
            _ => { // Fall
                (hip_l+Vec3::new(-0.05,-0.80,0.08), hip_r+Vec3::new(0.05,-0.80,0.08),
                 Vec3::new(-0.50,0.10,0.0), Vec3::new(0.50,0.10,0.0),
                 Vec3::new(-0.65,-0.05,0.0), Vec3::new(0.65,-0.05,0.0))
            }
        };
        let shoulder_l = chest + Vec3::new(-0.27,0.18,0.0);
        let shoulder_r = chest + Vec3::new(0.27,0.18,0.0);
        let knee_l = (hip_l+ankle_l)*0.5 + Vec3::new(0.0,0.0,-0.08);
        let knee_r = (hip_r+ankle_r)*0.5 + Vec3::new(0.0,0.0,-0.08);
        let joint_pos = |j: RJ| -> Vec3 {
            let local = match j {
                RJ::Pelvis=>pelvis, RJ::Chest=>chest, RJ::Neck=>neck, RJ::Head=>head,
                RJ::HipL=>hip_l, RJ::HipR=>hip_r, RJ::KneeL=>knee_l, RJ::KneeR=>knee_r,
                RJ::AnkleL=>ankle_l, RJ::AnkleR=>ankle_r, RJ::ShoulderL=>shoulder_l, RJ::ShoulderR=>shoulder_r,
                RJ::ElbowL=>elbow_l, RJ::ElbowR=>elbow_r, RJ::WristL=>wrist_l, RJ::WristR=>wrist_r,
            };
            root_pos + base * local
        };
        let frame = |f: RF| -> (Vec3, Quat) {
            match f {
                RF::Pelvis=>(root_pos, base),
                RF::Chest=>(joint_pos(RJ::Chest), base),
                RF::Head=>(joint_pos(RJ::Head), base),
                RF::FootL=>(joint_pos(RJ::AnkleL), base),
                RF::FootR=>(joint_pos(RJ::AnkleR), base),
                RF::HandL=>(joint_pos(RJ::WristL), base),
                RF::HandR=>(joint_pos(RJ::WristR), base),
            }
        };
        *vis = Visibility::Visible;
        match &part.spec {
            RSpec::Limb{a,b,r} => { *tf = r_limb(joint_pos(*a), joint_pos(*b), *r); }
            RSpec::Ball{j,r} => { *tf = Transform::from_translation(joint_pos(*j)).with_scale(Vec3::splat(r*2.0)); }
            RSpec::Rigid{f,off,rot,scale} => { let (p0,q)=frame(*f); *tf = Transform{ translation: p0+q*off, rotation: q*rot, scale: *scale }; }
            RSpec::Eye{off} => { let (p0,q)=frame(RF::Head); *tf = Transform{ translation: p0+q*off, rotation: q, scale: Vec3::new(0.055,0.075,0.02) }; }
            RSpec::Jet{foot,inner} => {
                if throttle <= 0.01 { *vis = Visibility::Hidden; continue; }
                let f = if *foot==0 { RF::FootL } else { RF::FootR };
                let (p0,q) = frame(f);
                let tsec = time.elapsed_secs();
                let phase2 = tsec*62.0 + (*foot as f32)*1.9;
                let flicker = 0.88 + 0.24*phase2.sin()*(phase2*1.7).cos();
                let len = (0.28 + throttle*0.34)*flicker*if *inner {0.62} else {1.0};
                let width = (0.09 + throttle.min(2.5)*0.035)*if *inner {0.58} else {1.0};
                let flip = Quat::from_rotation_x(std::f32::consts::PI);
                *tf = Transform{ translation: p0+q*Vec3::new(0.0,-0.10-len*0.5,-0.03), rotation: q*flip, scale: Vec3::new(width,len,width) };
            }
            RSpec::Canopy => {
                if boot==4 { *tf = Transform::from_translation(root_pos + base*Vec3::Y*6.5).with_rotation(base).with_scale(Vec3::new(4.5,1.6,2.6)); } else { *vis = Visibility::Hidden; }
            }
        }
    }
    // Wings
    for (w, m3, mut vis) in wings.iter_mut() {
        let Some((root_pos, root_rot, pitch, boot, _, wingsuit, _)) = avatars.get(&w.id).copied() else { *vis = Visibility::Hidden; continue };
        let flying = wingsuit && matches!(boot, 1|2|3);
        if !flying { *vis = Visibility::Hidden; continue; }
        *vis = Visibility::Visible;
        let base = root_rot * Quat::from_rotation_x(pitch*0.6);
        let back = base * Vec3::Z;
        // recompute joints quickly (approx)
        let chest = root_pos + base*Vec3::new(0.0,0.42,0.0);
        let shoulder_l = chest + base*Vec3::new(-0.27,0.18,0.0);
        let shoulder_r = chest + base*Vec3::new(0.27,0.18,0.0);
        let elbow_l = chest + base*Vec3::new(-0.57,0.18,0.0);
        let elbow_r = chest + base*Vec3::new(0.57,0.18,0.0);
        let wrist_l = chest + base*Vec3::new(-0.85,0.18,0.0);
        let wrist_r = chest + base*Vec3::new(0.85,0.18,0.0);
        let hip_l = root_pos + base*Vec3::new(-0.13,-0.04,0.0);
        let hip_r = root_pos + base*Vec3::new(0.13,-0.04,0.0);
        let ankle_l = root_pos + base*Vec3::new(-0.23,-0.79,0.15);
        let ankle_r = root_pos + base*Vec3::new(0.23,-0.79,0.15);
        let knee_l = (hip_l+ankle_l)*0.5 + base*Vec3::new(0.0,0.0,-0.08);
        let knee_r = (hip_r+ankle_r)*0.5 + base*Vec3::new(0.0,0.0,-0.08);
        let (poly, lift) = match w.k {
            0 => ([shoulder_l, elbow_l, wrist_l, (wrist_l+ankle_l)*0.5+back*0.03, ankle_l, knee_l, hip_l], back*0.05),
            1 => ([shoulder_r, elbow_r, wrist_r, (wrist_r+ankle_r)*0.5+back*0.03, ankle_r, knee_r, hip_r], back*0.05),
            _ => { let c=(hip_l+hip_r)*0.5 - base*Vec3::Y*0.04; ([hip_l,knee_l,ankle_l,ankle_r,knee_r,hip_r,c], back*0.035) }
        };
        let centroid = poly.iter().copied().sum::<Vec3>()/7.0 + lift;
        let n = (poly[2]-poly[0]).cross(poly[6]-poly[0]).normalize_or(back);
        let n = if n.dot(back)<0.0 {-n} else {n};
        if let Some(mut mesh) = meshes.get_mut(&m3.0) {
            let mut pos: Vec<[f32;3]> = poly.iter().map(|v| v.to_array()).collect();
            pos.push(centroid.to_array());
            mesh.insert_attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION, pos);
            mesh.insert_attribute(bevy::mesh::Mesh::ATTRIBUTE_NORMAL, vec![n.to_array();8]);
        }
    }
}