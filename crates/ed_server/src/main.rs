//! Earthdrop ghost-multiplayer relay server.
//!
//! Usage:
//!   earthdrop-server [port]            (default 7777)
//!   EARTH DROP server binds 0.0.0.0:<port>.
//!
//! It assigns each connecting client an id, relays PlayerState at ~30 Hz, and
//! drops clients that go silent for > 10 s. No world data is ever sent.

use ed_net::{C2S, PlayerInfo, S2C, DEFAULT_PORT, PROTOCOL_VERSION};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(10);

struct Client {
    addr: SocketAddr,
    name: String,
    last: Instant,
}

fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let sock = UdpSocket::bind(("0.0.0.0", port)).expect("bind");
    sock.set_nonblocking(true).expect("nonblocking");
    println!("earthdrop-server listening on 0.0.0.0:{port} (protocol {PROTOCOL_VERSION})");

    let clients: Arc<Mutex<HashMap<u64, Client>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut buf = [0u8; 4096];
    let mut next_id: u64 = 1;

    loop {
        // Receive.
        match sock.recv_from(&mut buf) {
            Ok((n, addr)) => {
                if let Some(msg) = ed_net::decode_c2s(&buf[..n]) {
                    match msg {
                        C2S::Hello { name, protocol } => {
                            if protocol != PROTOCOL_VERSION {
                                eprintln!("reject {addr}: protocol {protocol} != {PROTOCOL_VERSION}");
                                continue;
                            }
                            let id = next_id;
                            next_id += 1;
                            let mut cs = clients.lock().unwrap();
                            cs.insert(id, Client { addr, name: name.clone(), last: Instant::now() });
                            // Welcome with current roster.
                            let roster: Vec<PlayerInfo> = cs
                                .iter()
                                .filter(|(i, _)| **i != id)
                                .map(|(i, c)| PlayerInfo { id: *i, name: c.name.clone() })
                                .collect();
                            let w = ed_net::encode(&S2C::Welcome { id, players: roster });
                            let _ = sock.send_to(&w, addr);
                            // Announce join to others.
                            let j = ed_net::encode(&S2C::Join { id, name: name.clone() });
                            for (i, c) in cs.iter() {
                                if *i != id {
                                    let _ = sock.send_to(&j, c.addr);
                                }
                            }
                            println!("+ {name} ({addr}) id={id}; {} clients", cs.len());
                        }
                        C2S::State { pos, yaw, pitch, boot, throttle, wingsuit, t } => {
                            // find client by addr
                            let mut cs = clients.lock().unwrap();
                            let mut found = None;
                            for (i, c) in cs.iter_mut() {
                                if c.addr == addr {
                                    c.last = Instant::now();
                                    found = Some((*i, c.name.clone()));
                                    break;
                                }
                            }
                            if let Some((id, name)) = found {
                                let msg = ed_net::encode(&S2C::State {
                                    id,
                                    name,
                                    pos,
                                    yaw,
                                    pitch,
                                    boot,
                                    throttle,
                                    wingsuit,
                                    t,
                                });
                                // broadcast to everyone else
                                let addrs: Vec<SocketAddr> = cs
                                    .iter()
                                    .filter(|(i, _)| **i != id)
                                    .map(|(_, c)| c.addr)
                                    .collect();
                                for a in addrs {
                                    let _ = sock.send_to(&msg, a);
                                }
                            }
                        }
                        C2S::Bye => {
                            let mut cs = clients.lock().unwrap();
                            let mut to_remove = None;
                            for (i, c) in cs.iter() {
                                if c.addr == addr {
                                    to_remove = Some(*i);
                                    break;
                                }
                            }
                            if let Some(id) = to_remove {
                                cs.remove(&id);
                                let l = ed_net::encode(&S2C::Leave { id });
                                for (_, c) in cs.iter() {
                                    let _ = sock.send_to(&l, c.addr);
                                }
                                println!("- id={id}; {} clients", cs.len());
                            }
                        }
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => eprintln!("recv: {e}"),
        }

        // Timeouts.
        {
            let mut cs = clients.lock().unwrap();
            let now = Instant::now();
            let dead: Vec<u64> = cs
                .iter()
                .filter(|(_, c)| now.duration_since(c.last) > TIMEOUT)
                .map(|(i, _)| *i)
                .collect();
            for id in dead {
                cs.remove(&id);
                let l = ed_net::encode(&S2C::Leave { id });
                for (_, c) in cs.iter() {
                    let _ = sock.send_to(&l, c.addr);
                }
                println!("timeout id={id}; {} clients", cs.len());
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}