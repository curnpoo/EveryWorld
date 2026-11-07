//! Ghost multiplayer client.

use crate::config::Config;
use crate::origin::WorldOrigin;
use crate::player::Player;
use crate::spawn::SpawnRequest;
use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender};
use ed_geo::Geodetic;
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
    pub last: f64,
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
    State { pos: [f64; 3], yaw: f32, pitch: f32, boot: u8, t: f64 },
}

pub enum NetEvt {
    Connected { id: u64, players: Vec<(u64, String)> },
    Join { id: u64, name: String },
    State { id: u64, name: String, pos: [f64; 3], yaw: f32, pitch: f32, boot: u8, t: f64 },
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
        .add_systems(Update, (net_drain_events, net_send_state, net_spawn_avatars, net_update_avatars, net_update_status_text, net_menu_actions).chain());
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
                NetCmd::State { pos, yaw, pitch, boot, t } => {
                    if let (Some(s), Some(a)) = (&socket, server) {
                        let msg = encode(&C2S::State { pos: Vec3d::from(pos), yaw, pitch, boot, t });
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
                                S2C::State { id, name, pos, yaw, pitch, boot, t } => {
                                    let mut p = players.lock().unwrap();
                                    p.insert(id, RemotePlayer { id, name: name.clone(), pos: pos.into(), yaw, pitch, boot, last: t });
                                    let _ = evt_tx.send(NetEvt::State { id, name, pos: pos.into(), yaw, pitch, boot, t });
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
    mut spawn_writer: MessageWriter<SpawnRequest>,
) {
    let mut roster: Vec<(u64, String)> = vec![];
    let mut joins: Vec<(u64, String)> = vec![];
    let mut leaves: Vec<u64> = vec![];
    let mut states: Vec<(u64, String, [f64; 3], f32, f32, u8, f64)> = vec![];
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
            NetEvt::State { id, name, pos, yaw, pitch, boot, t } => states.push((id, name, pos, yaw, pitch, boot, t)),
            NetEvt::Leave { id } => leaves.push(id),
            NetEvt::Disconnected => { disconnected = true; }
        }
    }

    if connected_now {
        for (id, name) in roster {
            remote.players.insert(id, RemotePlayer { id, name, pos: [0.0; 3], yaw: 0.0, pitch: 0.0, boot: 0, last: 0.0 });
        }
    }
    for (id, name) in joins {
        remote.players.insert(id, RemotePlayer { id, name, pos: [0.0; 3], yaw: 0.0, pitch: 0.0, boot: 0, last: 0.0 });
    }
    let mut first_state_pos: Option<[f64; 3]> = None;
    for (id, name, pos, yaw, pitch, boot, t) in states {
        let rp = remote.players.entry(id).or_insert(RemotePlayer { id, name: name.clone(), pos, yaw, pitch, boot, last: t });
        rp.name = name; rp.pos = pos; rp.yaw = yaw; rp.pitch = pitch; rp.boot = boot; rp.last = t;
        if first_state_pos.is_none() { first_state_pos = Some(pos); }
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

    // Spawn next to a remote player the first time we receive their state.
    if client.status == NetStatus::Connected && !client.spawned_near {
        if let Some(pos) = first_state_pos {
            client.spawned_near = true;
            let geo = Geodetic::from_ecef(DVec3::new(pos[0], pos[1], pos[2]));
            spawn_writer.write(SpawnRequest::At { pos: geo, name: "remote".to_string(), height: crate::spawn::SpawnHeight::Ground });
        }
    }
}

fn net_send_state(
    client: Res<NetClient>,
    origin: Res<WorldOrigin>,
    q: Query<(&Transform, &Player), With<Player>>,
) {
    if client.status != NetStatus::Connected {
        return;
    }
    if let Ok((t, p)) = q.single() {
        let render = t.translation.as_dvec3();
        let ecef = origin.frame.render_to_ecef().transform_point3(render);
        let _ = client.cmd_tx.send(NetCmd::State {
            pos: [ecef.x, ecef.y, ecef.z],
            yaw: p.yaw,
            pitch: p.cam_pitch,
            boot: p.mode as u8,
            t: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0),
        });
    }
}

#[derive(Component)]
pub struct RemoteAvatar {
    pub id: u64,
}

#[derive(Component)]
pub struct RemoteAvatarRing {
    pub id: u64,
}

pub fn net_spawn_avatars(
    mut commands: Commands,
    mut meshes: ResMut<Assets<bevy::mesh::Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    remote: Res<RemotePlayers>,
    mut ents: ResMut<RemoteAvatarEntities>,
) {
    for (id, rp) in remote.players.iter() {
        if ents.map.contains_key(id) {
            continue;
        }
        let mesh = meshes.add(bevy::mesh::Mesh::from(Cuboid::new(0.6, 1.8, 0.4)));
        let mat = materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.6, 1.0), ..default() });
        let ring_mesh = meshes.add(bevy::mesh::Mesh::from(Cylinder::new(3.0, 0.15)));
        let ring_mat = materials.add(StandardMaterial { base_color: Color::srgba(0.2, 0.6, 1.0, 0.6), alpha_mode: AlphaMode::Blend, ..default() });
        let entity = commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::default(),
            RemoteAvatar { id: *id },
            crate::origin::Rebased,
            Name::new(format!("remote_{}", rp.name)),
        )).with_children(|p| {
            // nametag
            p.spawn((
                bevy::sprite::Text2d::new(rp.name.clone()),
                TextFont { font_size: bevy::text::FontSize::Px(14.0), ..default() },
                TextColor(Color::WHITE),
                Transform::from_translation(Vec3::new(0.0, 2.2, 0.0)),
            ));
            // far ring (hidden by default)
            p.spawn((
                Mesh3d(ring_mesh),
                MeshMaterial3d(ring_mat),
                Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                Visibility::Hidden,
                RemoteAvatarRing { id: *id },
            ));
        }).id();
        ents.map.insert(*id, entity);
    }
    // despawn removed
    let to_remove: Vec<u64> = ents.map.keys().copied().filter(|id| !remote.players.contains_key(id)).collect();
    for id in to_remove {
        if let Some(e) = ents.map.remove(&id) {
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

fn net_update_avatars(
    origin: Res<WorldOrigin>,
    remote: Res<RemotePlayers>,
    mut params: bevy::ecs::system::ParamSet<(
        Query<&Transform, With<Camera3d>>,
        Query<(&mut Transform, &RemoteAvatar, &Children), Without<RemoteAvatarRing>>,
        Query<&mut Visibility, (With<RemoteAvatarRing>, Without<RemoteAvatar>)>,
    )>,
) {
    let cam_pos = params.p0().iter().next().map(|t| t.translation).unwrap_or(Vec3::ZERO);
    let mut toggles: Vec<(Entity, bool)> = Vec::new();
    {
        let mut q = params.p1();
        for (mut t, av, children) in q.iter_mut() {
        if let Some(rp) = remote.players.get(&av.id) {
            let ecef = DVec3::new(rp.pos[0], rp.pos[1], rp.pos[2]);
            let render = origin.frame.ecef_to_render().transform_point3(ecef);
            t.translation = render.as_vec3();
            t.rotation = Quat::from_rotation_y(rp.yaw);
            let dist = (render.as_vec3() - cam_pos).length();
            let far = dist > 200.0;
            for c in children.iter() {
                toggles.push((c, far));
            }
            // hide avatar mesh when far? We'll keep avatar; ring shows location. Could hide avatar when very far.
        }
    }
    }
    {
        let mut ring_q = params.p2();
        for (e, far) in toggles {
            if let Ok(mut v) = ring_q.get_mut(e) {
                *v = if far { Visibility::Visible } else { Visibility::Hidden };
            }
        }
    }
}