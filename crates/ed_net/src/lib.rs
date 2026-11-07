//! Multiplayer wire protocol (ghost milestone).
//!
//! Engine-free: serde + serde_json only, so the server runs headless and the
//! client can reuse the types. One JSON object per UDP datagram.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 2;
pub const DEFAULT_PORT: u16 = 7777;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Vec3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl From<[f64; 3]> for Vec3d {
    fn from(a: [f64; 3]) -> Self {
        Self { x: a[0], y: a[1], z: a[2] }
    }
}
impl From<Vec3d> for [f64; 3] {
    fn from(v: Vec3d) -> Self {
        [v.x, v.y, v.z]
    }
}

/// Client -> server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum C2S {
    Hello { name: String, protocol: u32 },
    State {
        pos: Vec3d,
        yaw: f32,
        pitch: f32,
        boot: u8,
        throttle: f32,
        wingsuit: bool,
        t: f64,
    },
    Bye,
}

/// Server -> client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum S2C {
    Welcome { id: u64, players: Vec<PlayerInfo> },
    Join { id: u64, name: String },
    State {
        id: u64,
        name: String,
        pos: Vec3d,
        yaw: f32,
        pitch: f32,
        boot: u8,
        throttle: f32,
        wingsuit: bool,
        t: f64,
    },
    Leave { id: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: u64,
    pub name: String,
}

pub fn encode<T: Serialize>(v: &T) -> Vec<u8> {
    serde_json::to_vec(v).unwrap_or_default()
}

pub fn decode_c2s(bytes: &[u8]) -> Option<C2S> {
    serde_json::from_slice(bytes).ok()
}

pub fn decode_s2c(bytes: &[u8]) -> Option<S2C> {
    serde_json::from_slice(bytes).ok()
}