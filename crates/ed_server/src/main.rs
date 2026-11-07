//! Earthdrop ghost-multiplayer relay server.
//! Usage: earthdrop-server [port]

use ed_net::{C2S, PlayerInfo, S2C, DEFAULT_PORT, PROTOCOL_VERSION, Vec3d};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct StoredState { pos: Vec3d, yaw: f32, pitch: f32, boot: u8, throttle: f32, wingsuit: bool, t: f64 }

struct Client {
    addr: SocketAddr,
    name: String,
    last: Instant,
    state: Option<StoredState>,
    // per-peer last send time (interest management)
    last_sent: HashMap<u64, Instant>,
}

fn dist_m(a: Vec3d, b: Vec3d) -> f64 {
    let dx = a.x-b.x; let dy=a.y-b.y; let dz=a.z-b.z;
    (dx*dx+dy*dy+dz*dz).sqrt()
}
fn rate_for_dist(d: f64) -> Duration {
    if d < 1_000.0 { Duration::from_millis(16) }      // <1km: 60Hz
    else if d < 10_000.0 { Duration::from_millis(66) } // <10km: 15Hz
    else { Duration::from_millis(200) }                // far: 5Hz
}

fn main() {
    let port: u16 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(DEFAULT_PORT);
    let sock = UdpSocket::bind(("::", port)).expect("bind");
    sock.set_nonblocking(true).expect("nonblocking");
    println!("everyworld-server listening on [::]:{port} (protocol {PROTOCOL_VERSION})");
    let start_unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    let start_instant = Instant::now();

    let clients: Arc<Mutex<HashMap<u64, Client>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut buf = [0u8; 4096];
    let mut next_id: u64 = 1;
    let mut last_time_bcast = Instant::now();

    loop {
        match sock.recv_from(&mut buf) {
            Ok((n, addr)) => {
                if let Some(msg) = ed_net::decode_c2s(&buf[..n]) {
                    match msg {
                        C2S::Hello { name, protocol } => {
                            if protocol != PROTOCOL_VERSION {
                                eprintln!("reject {addr}: protocol {protocol} != {PROTOCOL_VERSION}");
                                continue;
                            }
                            let id = next_id; next_id += 1;
                            let mut cs = clients.lock().unwrap();
                            cs.insert(id, Client { addr, name: name.clone(), last: Instant::now(), state: None, last_sent: HashMap::new() });
                            let roster: Vec<PlayerInfo> = cs.iter().filter(|(i,_)| **i!=id).map(|(i,c)| PlayerInfo{id:*i,name:c.name.clone()}).collect();
                            let w = ed_net::encode_s2c(&S2C::Welcome{id,players:roster});
                            let _ = sock.send_to(&w, addr);
                            // immediate time sync
                            let now = start_unix + start_instant.elapsed().as_secs_f64();
                            let tm = ed_net::encode_s2c(&S2C::Time{unix_s: now, rate: 1.0});
                            let _ = sock.send_to(&tm, addr);
                            let j = ed_net::encode_s2c(&S2C::Join{id,name:name.clone()});
                            for (i,c) in cs.iter() { if *i!=id { let _ = sock.send_to(&j, c.addr); } }
                            println!("+ {name} ({addr}) id={id}; {} clients", cs.len());
                        }
                        C2S::State { pos, yaw, pitch, boot, throttle, wingsuit, t } => {
                            let mut cs = clients.lock().unwrap();
                            for (i,c) in cs.iter_mut() {
                                if c.addr==addr { c.last=Instant::now(); c.state=Some(StoredState{pos,yaw,pitch,boot,throttle,wingsuit,t}); break; }
                            }
                            // interest-managed broadcast: store, then dispatch below in tick
                        }
                        C2S::AdminTime { unix_s } => {
                            let cs = clients.lock().unwrap();
                            let tm = ed_net::encode_s2c(&S2C::Time{unix_s, rate: 1.0});
                            for (_,c) in cs.iter() { let _ = sock.send_to(&tm, c.addr); }
                        }
                        C2S::AdminWeather { preset } => {
                            let cs = clients.lock().unwrap();
                            let wm = ed_net::encode_s2c(&S2C::Weather{preset});
                            for (_,c) in cs.iter() { let _ = sock.send_to(&wm, c.addr); }
                        }
                        C2S::Bye => {
                            let mut cs = clients.lock().unwrap();
                            let mut to_remove=None;
                            for (i,c) in cs.iter() { if c.addr==addr { to_remove=Some(*i); break; } }
                            if let Some(id)=to_remove {
                                cs.remove(&id);
                                let l = ed_net::encode_s2c(&S2C::Leave{id});
                                for (_,c) in cs.iter() { let _ = sock.send_to(&l, c.addr); }
                                println!("- id={id}; {} clients", cs.len());
                            }
                        }
                    }
                }
            }
            Err(e) if e.kind()==std::io::ErrorKind::WouldBlock => {}
            Err(e) => eprintln!("recv: {e}"),
        }

        // interest-managed dispatch (60Hz tick, per-pair rate by distance)
        {
            let mut cs = clients.lock().unwrap();
            let now = Instant::now();
            // snapshot states
            let states: Vec<(u64, StoredState, String)> = cs.iter().filter_map(|(i,c)| c.state.clone().map(|s| (*i,s,c.name.clone()))).collect();
            // for each recipient, decide who to send
            let addrs: Vec<(u64, SocketAddr, Vec3d)> = cs.iter().map(|(i,c)| (*i,c.addr,c.state.as_ref().map(|s|s.pos).unwrap_or(Vec3d{x:0.0,y:0.0,z:0.0}))).collect();
            for (rid, raddr, rpos) in &addrs {
                for (sid, st, sname) in &states {
                    if sid==rid { continue; }
                    let d = dist_m(*rpos, st.pos);
                    let rate = rate_for_dist(d);
                    let last = cs.get(rid).and_then(|c| c.last_sent.get(sid)).copied();
                    if let Some(l) = last { if now.duration_since(l) < rate { continue; } }
                    let msg = ed_net::encode_s2c(&S2C::State{id:*sid,name:sname.clone(),pos:st.pos,yaw:st.yaw,pitch:st.pitch,boot:st.boot,throttle:st.throttle,wingsuit:st.wingsuit,t:st.t});
                    let _ = sock.send_to(&msg, *raddr);
                    if let Some(c) = cs.get_mut(rid) { c.last_sent.insert(*sid, now); }
                }
            }
        }

        // time broadcast 1Hz
        if last_time_bcast.elapsed() >= Duration::from_secs(1) {
            last_time_bcast = Instant::now();
            let now = start_unix + start_instant.elapsed().as_secs_f64();
            let tm = ed_net::encode_s2c(&S2C::Time{unix_s: now, rate: 1.0});
            let cs = clients.lock().unwrap();
            for (_,c) in cs.iter() { let _ = sock.send_to(&tm, c.addr); }
        }

        // timeouts
        {
            let mut cs = clients.lock().unwrap();
            let now = Instant::now();
            let dead: Vec<u64> = cs.iter().filter(|(_,c)| now.duration_since(c.last)>TIMEOUT).map(|(i,_)|*i).collect();
            for id in dead {
                cs.remove(&id);
                let l = ed_net::encode_s2c(&S2C::Leave{id});
                for (_,c) in cs.iter() { let _ = sock.send_to(&l, c.addr); }
                println!("timeout id={id}; {} clients", cs.len());
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
