//! Ghost multiplayer client.

use crate::body::{spawn_body_parts, BodyAssets, BodyOwner, Cosmetics, GaitStore, RigState};
use crate::config::Config;
use crate::origin::WorldOrigin;
use crate::player::{MoveMode, Player};
use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender};
use ed_net::{decode_s2c, encode_c2s, PosePart, Vec3d, C2S, PROTOCOL_VERSION, S2C};
use glam::DVec3;
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Resource, Default)]
pub struct RemotePlayers {
    pub players: HashMap<u64, RemotePlayer>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PoseMode {
    Procedural = 0,
    Replicated = 1,
}

impl PoseMode {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => PoseMode::Replicated,
            _ => PoseMode::Procedural,
        }
    }
}

/// Remote avatar proxy component. Stores replicated state and controls whether the rig
/// is procedural or driven by replicated part poses (ragdoll).
#[derive(Component)]
pub struct RemoteProxy {
    pub id: u64,
    pub state: RigState,
    pub quat: Quat,
    pub pose_mode: PoseMode,
    pub replicated_parts: HashMap<u16, (Vec3, Quat)>,
}

#[derive(Clone, Debug)]
pub struct RemotePlayer {
    pub id: u64,
    pub name: String,
    pub pos: [f64; 3],
    pub orient: [f32; 4],
    pub yaw: f32,
    pub cam_pitch: f32,
    pub boot: u8,
    pub throttle: f32,
    pub flags: u8,
    pub wing_arch: f32,
    pub wing_turn: f32,
    pub fall_spread: f32,
    pub fall_tuck: f32,
    pub strafe: f32,
    pub fwd_in: f32,
    pub jump_charge: f32,
    pub jump_push_timer: f32,
    pub landing_recovery: f32,
    pub rot_err: [f32; 3],
    pub intent_dir: [f32; 3],
    pub accel_smooth: [f32; 3],
    pub leg_steer: [f32; 3],
    pub pose_mode: u8,
    pub last: f64,
    // interpolation buffer
    pub prev_pos: [f64; 3],
    pub prev_orient: [f32; 4],
    pub prev_yaw: f32,
    pub prev_cam_pitch: f32,
    pub prev_boot: u8,
    pub prev_throttle: f32,
    pub prev_flags: u8,
    pub prev_wing_arch: f32,
    pub prev_wing_turn: f32,
    pub prev_fall_spread: f32,
    pub prev_fall_tuck: f32,
    pub prev_strafe: f32,
    pub prev_fwd_in: f32,
    pub prev_jump_charge: f32,
    pub prev_jump_push_timer: f32,
    pub prev_landing_recovery: f32,
    pub prev_rot_err: [f32; 3],
    pub prev_intent_dir: [f32; 3],
    pub prev_accel_smooth: [f32; 3],
    pub prev_leg_steer: [f32; 3],
    pub prev_pose_mode: u8,
    pub prev_t: f64,
}

impl RemotePlayer {
    pub fn empty(id: u64, name: String) -> Self {
        Self {
            id,
            name,
            pos: [0.0; 3],
            orient: [0.0, 0.0, 0.0, 1.0],
            yaw: 0.0,
            cam_pitch: 0.0,
            boot: 0,
            throttle: 0.0,
            flags: 0,
            wing_arch: 0.0,
            wing_turn: 0.0,
            fall_spread: 0.0,
            fall_tuck: 0.0,
            strafe: 0.0,
            fwd_in: 0.0,
            jump_charge: 0.0,
            jump_push_timer: 0.0,
            landing_recovery: 0.0,
            rot_err: [0.0; 3],
            intent_dir: [0.0; 3],
            accel_smooth: [0.0; 3],
            leg_steer: [0.0; 3],
            pose_mode: 0,
            last: 0.0,
            prev_pos: [0.0; 3],
            prev_orient: [0.0, 0.0, 0.0, 1.0],
            prev_yaw: 0.0,
            prev_cam_pitch: 0.0,
            prev_boot: 0,
            prev_throttle: 0.0,
            prev_flags: 0,
            prev_wing_arch: 0.0,
            prev_wing_turn: 0.0,
            prev_fall_spread: 0.0,
            prev_fall_tuck: 0.0,
            prev_strafe: 0.0,
            prev_fwd_in: 0.0,
            prev_jump_charge: 0.0,
            prev_jump_push_timer: 0.0,
            prev_landing_recovery: 0.0,
            prev_rot_err: [0.0; 3],
            prev_intent_dir: [0.0; 3],
            prev_accel_smooth: [0.0; 3],
            prev_leg_steer: [0.0; 3],
            prev_pose_mode: 0,
            prev_t: 0.0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_state(
        &mut self,
        name: String,
        pos: [f64; 3],
        orient: [f32; 4],
        yaw: f32,
        cam_pitch: f32,
        boot: u8,
        throttle: f32,
        flags: u8,
        wing_arch: f32,
        wing_turn: f32,
        fall_spread: f32,
        fall_tuck: f32,
        strafe: f32,
        fwd_in: f32,
        jump_charge: f32,
        jump_push_timer: f32,
        landing_recovery: f32,
        rot_err: [f32; 3],
        intent_dir: [f32; 3],
        accel_smooth: [f32; 3],
        leg_steer: [f32; 3],
        pose_mode: u8,
        t: f64,
    ) {
        self.prev_pos = self.pos;
        self.prev_orient = self.orient;
        self.prev_yaw = self.yaw;
        self.prev_cam_pitch = self.cam_pitch;
        self.prev_boot = self.boot;
        self.prev_throttle = self.throttle;
        self.prev_flags = self.flags;
        self.prev_wing_arch = self.wing_arch;
        self.prev_wing_turn = self.wing_turn;
        self.prev_fall_spread = self.fall_spread;
        self.prev_fall_tuck = self.fall_tuck;
        self.prev_strafe = self.strafe;
        self.prev_fwd_in = self.fwd_in;
        self.prev_jump_charge = self.jump_charge;
        self.prev_jump_push_timer = self.jump_push_timer;
        self.prev_landing_recovery = self.landing_recovery;
        self.prev_rot_err = self.rot_err;
        self.prev_intent_dir = self.intent_dir;
        self.prev_accel_smooth = self.accel_smooth;
        self.prev_leg_steer = self.leg_steer;
        self.prev_pose_mode = self.pose_mode;
        self.prev_t = self.last;

        self.name = name;
        self.pos = pos;
        self.orient = orient;
        self.yaw = yaw;
        self.cam_pitch = cam_pitch;
        self.boot = boot;
        self.throttle = throttle;
        self.flags = flags;
        self.wing_arch = wing_arch;
        self.wing_turn = wing_turn;
        self.fall_spread = fall_spread;
        self.fall_tuck = fall_tuck;
        self.strafe = strafe;
        self.fwd_in = fwd_in;
        self.jump_charge = jump_charge;
        self.jump_push_timer = jump_push_timer;
        self.landing_recovery = landing_recovery;
        self.rot_err = rot_err;
        self.intent_dir = intent_dir;
        self.accel_smooth = accel_smooth;
        self.leg_steer = leg_steer;
        self.pose_mode = pose_mode;
        self.last = t;
    }
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
    BadTarget,
}

pub enum NetCmd {
    Connect {
        addr: SocketAddr,
        name: String,
    },
    Disconnect,
    State {
        pos: [f64; 3],
        orient: [f32; 4],
        yaw: f32,
        cam_pitch: f32,
        boot: u8,
        throttle: f32,
        flags: u8,
        wing_arch: f32,
        wing_turn: f32,
        fall_spread: f32,
        fall_tuck: f32,
        strafe: f32,
        fwd_in: f32,
        jump_charge: f32,
        jump_push_timer: f32,
        landing_recovery: f32,
        rot_err: [f32; 3],
        intent_dir: [f32; 3],
        accel_smooth: [f32; 3],
        leg_steer: [f32; 3],
        pose_mode: u8,
        t: f64,
    },
    #[allow(dead_code)]
    Pose {
        seq: u32,
        parts: Vec<PosePart>,
    },
    AdminTime {
        unix_s: f64,
    },
    AdminWeather {
        preset: String,
    },
}

pub enum NetEvt {
    Connected {
        id: u64,
        players: Vec<(u64, String)>,
    },
    Join {
        id: u64,
        name: String,
    },
    State {
        id: u64,
        name: String,
        pos: [f64; 3],
        orient: [f32; 4],
        yaw: f32,
        cam_pitch: f32,
        boot: u8,
        throttle: f32,
        flags: u8,
        wing_arch: f32,
        wing_turn: f32,
        fall_spread: f32,
        fall_tuck: f32,
        strafe: f32,
        fwd_in: f32,
        jump_charge: f32,
        jump_push_timer: f32,
        landing_recovery: f32,
        rot_err: [f32; 3],
        intent_dir: [f32; 3],
        accel_smooth: [f32; 3],
        leg_steer: [f32; 3],
        pose_mode: u8,
        t: f64,
    },
    Pose {
        id: u64,
        seq: u32,
        parts: Vec<PosePart>,
    },
    Leave {
        id: u64,
    },
    Time {
        unix_s: f64,
        rate: f64,
    },
    Weather {
        preset: String,
    },
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
        if let Some(mut form) = app.world_mut().get_resource_mut::<crate::menu::JoinForm>() {
            form.server = target.clone();
            form.name = name.clone();
        }

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
        .add_systems(Update, (join_input, join_paint, net_drain_events, net_send_state, net_spawn_avatars, net_update_avatars, net_update_nametags, net_update_status_text, net_menu_actions).chain());
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
                    let bind_addr = if addr.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
                    let s = UdpSocket::bind(bind_addr).ok();
                    if let Some(s) = &s {
                        s.set_nonblocking(true).ok();
                        let hello = encode_c2s(&C2S::Hello { name: "player".to_string(), protocol: PROTOCOL_VERSION });
                        let _ = s.send_to(&hello, addr);
                    }
                    socket = s;
                    server = Some(addr);
                }
                NetCmd::Disconnect => {
                    if let (Some(s), Some(a)) = (&socket, server) {
                        let _ = s.send_to(&encode_c2s(&C2S::Bye), a);
                    }
                    socket = None;
                    server = None;
                    let _ = evt_tx.send(NetEvt::Disconnected);
                    players.lock().unwrap().clear();
                }
                NetCmd::State {
                    pos,
                    orient,
                    yaw,
                    cam_pitch,
                    boot,
                    throttle,
                    flags,
                    wing_arch,
                    wing_turn,
                    fall_spread,
                    fall_tuck,
                    strafe,
                    fwd_in,
                    jump_charge,
                    jump_push_timer,
                    landing_recovery,
                    rot_err,
                    intent_dir,
                    accel_smooth,
                    leg_steer,
                    pose_mode,
                    t,
                } => {
                    if let (Some(s), Some(a)) = (&socket, server) {
                        let msg = encode_c2s(&C2S::State {
                            pos: Vec3d::from(pos),
                            orient,
                            yaw,
                            cam_pitch,
                            boot,
                            throttle,
                            flags,
                            wing_arch,
                            wing_turn,
                            fall_spread,
                            fall_tuck,
                            strafe,
                            fwd_in,
                            jump_charge,
                            jump_push_timer,
                            landing_recovery,
                            rot_err,
                            intent_dir,
                            accel_smooth,
                            leg_steer,
                            pose_mode,
                            t,
                        });
                        let _ = s.send_to(&msg, a);
                    }
                }
                NetCmd::Pose { seq, parts } => {
                    if let (Some(s), Some(a)) = (&socket, server) {
                        let msg = encode_c2s(&C2S::Pose { seq, parts });
                        let _ = s.send_to(&msg, a);
                    }
                }
                NetCmd::AdminTime { unix_s } => {
                    if let (Some(s), Some(a)) = (&socket, server) {
                        let msg = encode_c2s(&C2S::AdminTime { unix_s });
                        let _ = s.send_to(&msg, a);
                    }
                }
                NetCmd::AdminWeather { preset } => {
                    if let (Some(s), Some(a)) = (&socket, server) {
                        let msg = encode_c2s(&C2S::AdminWeather { preset });
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
                                S2C::State {
                                    id,
                                    name,
                                    pos,
                                    orient,
                                    yaw,
                                    cam_pitch,
                                    boot,
                                    throttle,
                                    flags,
                                    wing_arch,
                                    wing_turn,
                                    fall_spread,
                                    fall_tuck,
                                    strafe,
                                    fwd_in,
                                    jump_charge,
                                    jump_push_timer,
                                    landing_recovery,
                                    rot_err,
                                    intent_dir,
                                    accel_smooth,
                                    leg_steer,
                                    pose_mode,
                                    t,
                                } => {
                                    let mut p = players.lock().unwrap();
                                    let rp = p.entry(id).or_insert_with(|| RemotePlayer::empty(id, name.clone()));
                                    rp.update_state(
                                        name.clone(),
                                        pos.into(),
                                        orient,
                                        yaw,
                                        cam_pitch,
                                        boot,
                                        throttle,
                                        flags,
                                        wing_arch,
                                        wing_turn,
                                        fall_spread,
                                        fall_tuck,
                                        strafe,
                                        fwd_in,
                                        jump_charge,
                                        jump_push_timer,
                                        landing_recovery,
                                        rot_err,
                                        intent_dir,
                                        accel_smooth,
                                        leg_steer,
                                        pose_mode,
                                        t,
                                    );
                                    let _ = evt_tx.send(NetEvt::State {
                                        id,
                                        name,
                                        pos: pos.into(),
                                        orient,
                                        yaw,
                                        cam_pitch,
                                        boot,
                                        throttle,
                                        flags,
                                        wing_arch,
                                        wing_turn,
                                        fall_spread,
                                        fall_tuck,
                                        strafe,
                                        fwd_in,
                                        jump_charge,
                                        jump_push_timer,
                                        landing_recovery,
                                        rot_err,
                                        intent_dir,
                                        accel_smooth,
                                        leg_steer,
                                        pose_mode,
                                        t,
                                    });
                                }
                                S2C::Pose { id, seq, parts } => {
                                    let _ = evt_tx.send(NetEvt::Pose { id, seq, parts });
                                }
                                S2C::Leave { id } => {
                                    players.lock().unwrap().remove(&id);
                                    let _ = evt_tx.send(NetEvt::Leave { id });
                                }
                                S2C::Time { unix_s, rate } => {
                                    let _ = evt_tx.send(NetEvt::Time { unix_s, rate });
                                }
                                S2C::Weather { preset } => {
                                    let _ = evt_tx.send(NetEvt::Weather { preset });
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
    mut proxies: Query<&mut RemoteProxy>,
    mut clock: ResMut<crate::sky::Clock>,
    mut weather: ResMut<crate::sky::CurrentWeather>,
    mut over: ResMut<crate::clouds::CloudOverride>,
) {
    let mut roster: Vec<(u64, String)> = vec![];
    let mut joins: Vec<(u64, String)> = vec![];
    let mut leaves: Vec<u64> = vec![];
    let mut states = vec![];
    let mut poses = vec![];
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
            NetEvt::State {
                id,
                name,
                pos,
                orient,
                yaw,
                cam_pitch,
                boot,
                throttle,
                flags,
                wing_arch,
                wing_turn,
                fall_spread,
                fall_tuck,
                strafe,
                fwd_in,
                jump_charge,
                jump_push_timer,
                landing_recovery,
                rot_err,
                intent_dir,
                accel_smooth,
                leg_steer,
                pose_mode,
                t,
            } => states.push((
                id,
                name,
                pos,
                orient,
                yaw,
                cam_pitch,
                boot,
                throttle,
                flags,
                wing_arch,
                wing_turn,
                fall_spread,
                fall_tuck,
                strafe,
                fwd_in,
                jump_charge,
                jump_push_timer,
                landing_recovery,
                rot_err,
                intent_dir,
                accel_smooth,
                leg_steer,
                pose_mode,
                t,
            )),
            NetEvt::Pose { id, seq, parts } => poses.push((id, seq, parts)),
            NetEvt::Leave { id } => leaves.push(id),
            NetEvt::Time { unix_s, rate } => {
                clock.unix_s = unix_s;
                clock.rate = rate;
            }
            NetEvt::Weather { preset } => {
                if let Some(w) = ed_world::clouds::weather_preset(&preset) {
                    weather.0 = Some(w);
                    over.set_changed();
                }
            }
            NetEvt::Disconnected => {
                disconnected = true;
            }
        }
    }

    if connected_now {
        for (id, name) in roster {
            remote.players.insert(id, RemotePlayer::empty(id, name));
        }
    }
    for (id, name) in joins {
        remote.players.insert(id, RemotePlayer::empty(id, name));
    }
    for (
        id,
        name,
        pos,
        orient,
        yaw,
        cam_pitch,
        boot,
        throttle,
        flags,
        wing_arch,
        wing_turn,
        fall_spread,
        fall_tuck,
        strafe,
        fwd_in,
        jump_charge,
        jump_push_timer,
        landing_recovery,
        rot_err,
        intent_dir,
        accel_smooth,
        leg_steer,
        pose_mode,
        t,
    ) in states
    {
        let rp = remote.players.entry(id).or_insert_with(|| RemotePlayer::empty(id, name.clone()));
        rp.update_state(
            name,
            pos,
            orient,
            yaw,
            cam_pitch,
            boot,
            throttle,
            flags,
            wing_arch,
            wing_turn,
            fall_spread,
            fall_tuck,
            strafe,
            fwd_in,
            jump_charge,
            jump_push_timer,
            landing_recovery,
            rot_err,
            intent_dir,
            accel_smooth,
            leg_steer,
            pose_mode,
            t,
        );
    }
    for (id, _seq, parts) in poses {
        for mut proxy in &mut proxies {
            if proxy.id == id {
                proxy.pose_mode = PoseMode::Replicated;
                proxy.replicated_parts.clear();
                for p in parts {
                    proxy.replicated_parts.insert(
                        p.index,
                        (Vec3::from(p.pos), Quat::from_array(p.rot)),
                    );
                }
                break;
            }
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
        let orient = origin.frame.quat_render_to_ecef(t.rotation).to_array();
        let rot_err = origin
            .frame
            .dir_render_to_ecef(p.rot_err.as_dvec3())
            .as_vec3()
            .to_array();
        let intent_dir = origin
            .frame
            .dir_render_to_ecef(p.intent_dir.as_dvec3())
            .as_vec3()
            .to_array();
        let accel_smooth = origin
            .frame
            .dir_render_to_ecef(p.accel_smooth.as_dvec3())
            .as_vec3()
            .to_array();
        let leg_steer = p.leg_steer.to_array();

        let mut flags = 0u8;
        if p.wingsuit_on {
            flags |= 1;
        }
        if p.afterburner {
            flags |= 2;
        }
        if p.crouch {
            flags |= 4;
        }

        let _ = client.cmd_tx.send(NetCmd::State {
            pos: [ecef.x, ecef.y, ecef.z],
            orient,
            yaw: p.yaw,
            cam_pitch: p.cam_pitch,
            boot: p.mode as u8,
            throttle: p.throttle,
            flags,
            wing_arch: p.wing_arch,
            wing_turn: p.wing_turn,
            fall_spread: p.fall_spread,
            fall_tuck: p.fall_tuck,
            strafe: p.strafe,
            fwd_in: p.fwd_in,
            jump_charge: p.jump_charge,
            jump_push_timer: p.jump_push_timer,
            landing_recovery: p.landing_recovery,
            rot_err,
            intent_dir,
            accel_smooth,
            leg_steer,
            pose_mode: 0,
            t: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0),
        });
    }
}

#[derive(Component)]
pub struct RemoteAvatar {
    pub id: u64,
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
    body_assets: Res<BodyAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    remote: Res<RemotePlayers>,
    mut ents: ResMut<RemoteAvatarEntities>,
    mut nametag_entities: ResMut<RemoteNametagEntities>,
    parts_query: Query<(Entity, &BodyOwner)>,
    mut gait_store: ResMut<GaitStore>,
) {
    for (id, rp) in remote.players.iter() {
        if ents.map.contains_key(id) {
            continue;
        }
        let initial_state = RigState {
            mode: MoveMode::from_u8(rp.boot),
            yaw: rp.yaw,
            cam_pitch: rp.cam_pitch,
            throttle: rp.throttle,
            wingsuit_on: (rp.flags & 1) != 0,
            afterburner: (rp.flags & 2) != 0,
            fwd_in: rp.fwd_in,
            strafe: rp.strafe,
            wing_arch: rp.wing_arch,
            wing_turn: rp.wing_turn,
            fall_spread: rp.fall_spread,
            fall_tuck: rp.fall_tuck,
            jump_charge: rp.jump_charge,
            jump_push_timer: rp.jump_push_timer,
            crouch: (rp.flags & 4) != 0,
            landing_recovery: rp.landing_recovery,
            leg_steer: Vec3::from(rp.leg_steer),
            intent_dir: Vec3::from(rp.intent_dir),
            rot_err: Vec3::from(rp.rot_err),
            accel_smooth: Vec3::from(rp.accel_smooth),
            ground_normal: Vec3::Y,
            foot_ground_hit: [None, None],
            first_person: false,
            vel: Vec3::ZERO,
        };

        let entity = commands
            .spawn((
                Transform::default(),
                RemoteAvatar { id: *id },
                RemoteProxy {
                    id: *id,
                    state: initial_state,
                    quat: Quat::IDENTITY,
                    pose_mode: PoseMode::from_u8(rp.pose_mode),
                    replicated_parts: HashMap::new(),
                },
                crate::origin::Rebased,
                Name::new(format!("remote_{}", rp.name)),
            ))
            .id();

        spawn_body_parts(
            &mut commands,
            &body_assets,
            &mut meshes,
            entity,
            &Cosmetics { backpack: false },
        );

        // screen-space nametag with box
        let nametag = commands
            .spawn((
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
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(rp.name.clone()),
                    TextFont {
                        font_size: bevy::text::FontSize::Px(18.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.9, 0.2)),
                ));
            })
            .id();
        ents.map.insert(*id, entity);
        nametag_entities.map.insert(*id, nametag);
    }
    let to_remove: Vec<u64> = ents
        .map
        .keys()
        .copied()
        .filter(|id| !remote.players.contains_key(id))
        .collect();
    for id in to_remove {
        if let Some(e) = ents.map.remove(&id) {
            for (part_ent, owner) in &parts_query {
                if owner.0 == e {
                    commands.entity(part_ent).despawn();
                }
            }
            gait_store.0.remove(&e);
            commands.entity(e).despawn();
        }
        if let Some(e) = nametag_entities.map.remove(&id) {
            commands.entity(e).despawn();
        }
    }
}

fn join_input(
    mut keys: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut form: ResMut<crate::menu::JoinForm>,
    mut net: ResMut<NetClient>,
    mut clipboard: ResMut<bevy_clipboard::prelude::Clipboard>,
    keys_pressed: Res<ButtonInput<bevy::input::keyboard::KeyCode>>,
    mut server_q: Query<&Interaction, With<crate::menu::ServerField>>,
    mut name_q: Query<&Interaction, With<crate::menu::NameField>>,
) {
    // click to focus
    for i in &server_q {
        if *i == Interaction::Pressed { form.focus = 1; }
    }
    for i in &name_q {
        if *i == Interaction::Pressed { form.focus = 2; }
    }
    // Paste via Cmd+V (Mac) / Ctrl+V (Windows/Linux)
    let paste = keys_pressed.just_pressed(bevy::input::keyboard::KeyCode::KeyV)
        && (keys_pressed.pressed(bevy::input::keyboard::KeyCode::SuperLeft)
            || keys_pressed.pressed(bevy::input::keyboard::KeyCode::SuperRight)
            || keys_pressed.pressed(bevy::input::keyboard::KeyCode::ControlLeft)
            || keys_pressed.pressed(bevy::input::keyboard::KeyCode::ControlRight));
    if paste && form.focus != 0 {
        if let bevy_clipboard::prelude::ClipboardRead::Ready(Ok(text)) = clipboard.fetch_text() {
            let text = text.trim().to_string();
            if form.focus == 1 {
                if form.server.len() + text.len() <= 64 { form.server.push_str(&text); }
            } else if form.name.len() + text.len() <= 32 {
                form.name.push_str(&text);
            }
        }
    }
    if form.focus == 0 { return; }
    for key in keys.read() {
        if key.state != bevy::input::ButtonState::Pressed { continue; }
        match key.key_code {
            bevy::input::keyboard::KeyCode::Escape => form.focus = 0,
            bevy::input::keyboard::KeyCode::Backspace => {
                if form.focus == 1 { form.server.pop(); } else { form.name.pop(); }
            }
            bevy::input::keyboard::KeyCode::Enter => {
                form.focus = 0;
                net.target = form.server.clone();
                net.name = form.name.clone();
                if let Ok(addr) = net.target.parse() {
                    net.cmd_tx.send(NetCmd::Connect { addr, name: net.name.clone() }).ok();
                    net.status = NetStatus::Connecting;
                } else {
                    net.status = NetStatus::BadTarget;
                }
            }
            bevy::input::keyboard::KeyCode::Tab => { form.focus = if form.focus == 1 { 2 } else { 1 }; }
            _ => {
                let mod_held = keys_pressed.pressed(bevy::input::keyboard::KeyCode::SuperLeft)
                    || keys_pressed.pressed(bevy::input::keyboard::KeyCode::SuperRight)
                    || keys_pressed.pressed(bevy::input::keyboard::KeyCode::ControlLeft)
                    || keys_pressed.pressed(bevy::input::keyboard::KeyCode::ControlRight);
                if mod_held { continue; }
                if let Some(s) = &key.text {
                    for c in s.chars() {
                        if c.is_ascii_graphic() || c == ' ' {
                            if form.focus == 1 { if form.server.len() < 64 { form.server.push(c); } }
                            else if form.name.len() < 32 { form.name.push(c); }
                        }
                    }
                }
            }
        }
    }
}

fn join_paint(
    form: Res<crate::menu::JoinForm>,
    mut server_t: Query<&mut Text, (With<crate::menu::ServerFieldText>, Without<crate::menu::NameFieldText>)>,
    mut name_t: Query<&mut Text, (With<crate::menu::NameFieldText>, Without<crate::menu::ServerFieldText>)>,
) {
    let caret = |f: u8, my: u8| if f == my { "│" } else { "" };
    for mut t in &mut server_t { t.0 = format!("{}{}", form.server, caret(form.focus, 1)); }
    for mut t in &mut name_t { t.0 = format!("{}{}", form.name, caret(form.focus, 2)); }
}

pub fn net_menu_actions(
    q: Query<(&Interaction, &crate::menu::MenuAction), Changed<Interaction>>,
    mut net: ResMut<NetClient>,
    form: Res<crate::menu::JoinForm>,
    clock: Res<crate::sky::Clock>,
) {
    for (i, a) in q.iter() {
        if *i != Interaction::Pressed {
            continue;
        }
        match a {
            crate::menu::MenuAction::ConnectServer => {
                net.target = form.server.clone();
                net.name = form.name.clone();
                if let Ok(addr) = net.target.parse() {
                    net.cmd_tx.send(NetCmd::Connect { addr, name: net.name.clone() }).ok();
                    net.status = NetStatus::Connecting;
                } else {
                    net.status = NetStatus::BadTarget;
                }
            }
            crate::menu::MenuAction::DisconnectServer => {
                net.cmd_tx.send(NetCmd::Disconnect).ok();
                net.status = NetStatus::Disconnected;
            }
            crate::menu::MenuAction::AdminTimeMinus => { net.cmd_tx.send(NetCmd::AdminTime { unix_s: clock.unix_s - 3600.0 }).ok(); }
            crate::menu::MenuAction::AdminTimePlus => { net.cmd_tx.send(NetCmd::AdminTime { unix_s: clock.unix_s + 3600.0 }).ok(); }
            crate::menu::MenuAction::AdminTimeNow => {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(clock.unix_s);
                net.cmd_tx.send(NetCmd::AdminTime { unix_s: now }).ok();
            }
            crate::menu::MenuAction::AdminWeather(preset) => { net.cmd_tx.send(NetCmd::AdminWeather { preset: preset.to_string() }).ok(); }
            _ => {}
        }
    }
}

fn net_update_nametags(
    remote: Res<RemotePlayers>,
    camera: Query<(&GlobalTransform, &Camera), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    avatars: Query<(&Transform, &RemoteAvatar)>,
    mut q: Query<(&mut Node, &RemoteNametag, &mut Visibility)>,
) {
    let cam = camera.iter().next();
    let pos_by_id: HashMap<u64, Vec3> = avatars.iter().map(|(t, a)| (a.id, t.translation)).collect();
    // Scale factor between the (possibly downscaled) 3D render target and the
    // native window so nametags stay aligned with the world at any resolution.
    let win_logical = windows.single().ok().map(|w| w.resolution.size()).unwrap_or(Vec2::ZERO);
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
            let scale = if let Some(cam_size) = cam_c.logical_viewport_size() {
                if cam_size.x > 0.0 && cam_size.y > 0.0 && win_logical.x > 0.0 {
                    win_logical / cam_size
                } else {
                    Vec2::ONE
                }
            } else {
                Vec2::ONE
            };
            node.left = Val::Px(screen.x * scale.x - 40.0);
            node.top = Val::Px(screen.y * scale.y - 170.0);
            *vis = Visibility::Visible;
        } else {
            *vis = Visibility::Hidden;
        }
    }
}

fn net_update_status_text(
    client: Res<NetClient>,
    remote: Res<RemotePlayers>,
    mut q: Query<&mut Text, With<crate::menu::NetStatusText>>,
) {
    let s = match client.status {
        NetStatus::Disconnected => format!("Disconnected (target {})", client.target),
        NetStatus::Connecting => format!("Connecting to {}...", client.target),
        NetStatus::Connected => {
            let names: Vec<String> = remote.players.values().map(|p| p.name.clone()).collect();
            if names.is_empty() {
                format!("Connected as {} (id {:?}) — solo", client.name, client.id)
            } else {
                format!("Connected as {} (id {:?}) — {}: {}", client.name, client.id, names.len(), names.join(", "))
            }
        }
        NetStatus::BadTarget => format!("Bad address '{}' — use host:port (IPv6 needs brackets: [2001:db8::1]:7777)", client.target),
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
    mut q: Query<(&mut Transform, &mut RemoteProxy, &RemoteAvatar)>,
) {
    for (mut t, mut proxy, av) in q.iter_mut() {
        let Some(rp) = remote.players.get(&av.id) else {
            continue;
        };
        // interpolation + dead reckoning + jitter-adaptive buffer
        let span = (rp.last - rp.prev_t).max(1e-4);
        let jitter = ((span - 1.0 / 60.0).abs()).min(0.05);
        let buf = (0.03 + jitter * 2.0).clamp(0.016, 0.1);
        let render_t = rp.last - buf;
        let alpha_raw = (render_t - rp.prev_t) / span;
        let (alpha, extrapolate): (f32, f64) = if alpha_raw > 1.0 {
            // packet loss: dead-reckon past curr using velocity
            let over = (alpha_raw - 1.0) * span; // seconds beyond curr
            let over_capped = over.min(0.5);
            (1.0, over_capped)
        } else {
            (alpha_raw.clamp(0.0, 1.0) as f32, 0.0)
        };
        let vel = [
            (rp.pos[0] - rp.prev_pos[0]) / span,
            (rp.pos[1] - rp.prev_pos[1]) / span,
            (rp.pos[2] - rp.prev_pos[2]) / span,
        ];
        let pos = [
            rp.prev_pos[0] + (rp.pos[0] - rp.prev_pos[0]) * alpha as f64 + vel[0] * extrapolate,
            rp.prev_pos[1] + (rp.pos[1] - rp.prev_pos[1]) * alpha as f64 + vel[1] * extrapolate,
            rp.prev_pos[2] + (rp.pos[2] - rp.prev_pos[2]) * alpha as f64 + vel[2] * extrapolate,
        ];
        let ecef = DVec3::new(pos[0], pos[1], pos[2]);
        let render = origin.frame.ecef_to_render().transform_point3(ecef).as_vec3();

        let q_prev = Quat::from_array(rp.prev_orient);
        let q_curr = Quat::from_array(rp.orient);
        let q_ecef = if q_prev.length_squared() > 0.5 && q_curr.length_squared() > 0.5 {
            q_prev.slerp(q_curr, alpha).normalize()
        } else {
            Quat::from_rotation_y(lerp_angle(rp.prev_yaw, rp.yaw, alpha))
        };
        let quat_render = origin.frame.quat_ecef_to_render(q_ecef);

        let vel_render = origin
            .frame
            .dir_ecef_to_render(DVec3::new(vel[0], vel[1], vel[2]))
            .as_vec3();

        let rot_err_ecef = DVec3::new(
            (rp.prev_rot_err[0] + (rp.rot_err[0] - rp.prev_rot_err[0]) * alpha) as f64,
            (rp.prev_rot_err[1] + (rp.rot_err[1] - rp.prev_rot_err[1]) * alpha) as f64,
            (rp.prev_rot_err[2] + (rp.rot_err[2] - rp.prev_rot_err[2]) * alpha) as f64,
        );
        let rot_err = origin.frame.dir_ecef_to_render(rot_err_ecef).as_vec3();

        let intent_dir_ecef = DVec3::new(
            (rp.prev_intent_dir[0] + (rp.intent_dir[0] - rp.prev_intent_dir[0]) * alpha) as f64,
            (rp.prev_intent_dir[1] + (rp.intent_dir[1] - rp.prev_intent_dir[1]) * alpha) as f64,
            (rp.prev_intent_dir[2] + (rp.intent_dir[2] - rp.prev_intent_dir[2]) * alpha) as f64,
        );
        let intent_dir = origin.frame.dir_ecef_to_render(intent_dir_ecef).as_vec3();

        let accel_smooth_ecef = DVec3::new(
            (rp.prev_accel_smooth[0] + (rp.accel_smooth[0] - rp.prev_accel_smooth[0]) * alpha) as f64,
            (rp.prev_accel_smooth[1] + (rp.accel_smooth[1] - rp.prev_accel_smooth[1]) * alpha) as f64,
            (rp.prev_accel_smooth[2] + (rp.accel_smooth[2] - rp.prev_accel_smooth[2]) * alpha) as f64,
        );
        let accel_smooth = origin.frame.dir_ecef_to_render(accel_smooth_ecef).as_vec3();

        let leg_steer = Vec3::new(
            rp.prev_leg_steer[0] + (rp.leg_steer[0] - rp.prev_leg_steer[0]) * alpha,
            rp.prev_leg_steer[1] + (rp.leg_steer[1] - rp.prev_leg_steer[1]) * alpha,
            rp.prev_leg_steer[2] + (rp.leg_steer[2] - rp.prev_leg_steer[2]) * alpha,
        );

        let yaw = lerp_angle(rp.prev_yaw, rp.yaw, alpha);
        let cam_pitch = rp.prev_cam_pitch + (rp.cam_pitch - rp.prev_cam_pitch) * alpha;
        let boot = if alpha > 0.5 { rp.boot } else { rp.prev_boot };
        let flags = if alpha > 0.5 { rp.flags } else { rp.prev_flags };
        let pose_mode = if alpha > 0.5 { rp.pose_mode } else { rp.prev_pose_mode };
        let throttle = rp.prev_throttle + (rp.throttle - rp.prev_throttle) * alpha;
        let wing_arch = rp.prev_wing_arch + (rp.wing_arch - rp.prev_wing_arch) * alpha;
        let wing_turn = rp.prev_wing_turn + (rp.wing_turn - rp.prev_wing_turn) * alpha;
        let fall_spread = rp.prev_fall_spread + (rp.fall_spread - rp.prev_fall_spread) * alpha;
        let fall_tuck = rp.prev_fall_tuck + (rp.fall_tuck - rp.prev_fall_tuck) * alpha;
        let strafe = rp.prev_strafe + (rp.strafe - rp.prev_strafe) * alpha;
        let fwd_in = rp.prev_fwd_in + (rp.fwd_in - rp.prev_fwd_in) * alpha;
        let jump_charge = rp.prev_jump_charge + (rp.jump_charge - rp.prev_jump_charge) * alpha;
        let jump_push_timer = rp.prev_jump_push_timer + (rp.jump_push_timer - rp.prev_jump_push_timer) * alpha;
        let landing_recovery = rp.prev_landing_recovery + (rp.landing_recovery - rp.prev_landing_recovery) * alpha;

        t.translation = render;
        t.rotation = quat_render;

        proxy.quat = quat_render;
        proxy.pose_mode = PoseMode::from_u8(pose_mode);
        proxy.state = RigState {
            mode: MoveMode::from_u8(boot),
            yaw,
            cam_pitch,
            throttle,
            wingsuit_on: (flags & 1) != 0,
            afterburner: (flags & 2) != 0,
            fwd_in,
            strafe,
            wing_arch,
            wing_turn,
            fall_spread,
            fall_tuck,
            jump_charge,
            jump_push_timer,
            crouch: (flags & 4) != 0,
            landing_recovery,
            leg_steer,
            intent_dir,
            rot_err,
            accel_smooth,
            ground_normal: Vec3::Y,
            foot_ground_hit: [None, None],
            first_person: false,
            vel: vel_render,
        };
    }
}