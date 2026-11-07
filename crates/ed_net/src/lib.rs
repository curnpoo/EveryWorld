//! Multiplayer wire protocol (binary, compact).
//!
//! Engine-free. Compact binary encoding (no JSON): ~150 bytes per State.

pub const PROTOCOL_VERSION: u32 = 5;
pub const DEFAULT_PORT: u16 = 7777;

#[derive(Debug, Clone, Copy, PartialEq)]
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

/// One part transform in owner-local coordinates for ragdoll pose replication.
#[derive(Debug, Clone, PartialEq)]
pub struct PosePart {
    pub index: u16,
    pub pos: [f32; 3], // owner-local meters
    pub rot: [f32; 4], // owner-local quaternion
}

#[derive(Debug, Clone)]
pub enum C2S {
    Hello {
        name: String,
        protocol: u32,
    },
    State {
        pos: Vec3d,
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
        seq: u32,
        parts: Vec<PosePart>,
    },
    AdminTime {
        unix_s: f64,
    },
    AdminWeather {
        preset: String,
    },
    Bye,
}

#[derive(Debug, Clone)]
pub enum S2C {
    Welcome {
        id: u64,
        players: Vec<PlayerInfo>,
    },
    Join {
        id: u64,
        name: String,
    },
    State {
        id: u64,
        name: String,
        pos: Vec3d,
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
}

#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub id: u64,
    pub name: String,
}

// ---- binary helpers ----
fn w_u8(v: &mut Vec<u8>, x: u8) {
    v.push(x)
}
fn w_u16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes())
}
fn w_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes())
}
fn w_u64(v: &mut Vec<u8>, x: u64) {
    v.extend_from_slice(&x.to_le_bytes())
}
fn w_f32(v: &mut Vec<u8>, x: f32) {
    v.extend_from_slice(&x.to_le_bytes())
}
fn w_f64(v: &mut Vec<u8>, x: f64) {
    v.extend_from_slice(&x.to_le_bytes())
}
fn w_str(v: &mut Vec<u8>, s: &str) {
    v.push(s.len() as u8);
    v.extend_from_slice(s.as_bytes())
}
fn w_vec3d(v: &mut Vec<u8>, p: Vec3d) {
    w_f64(v, p.x);
    w_f64(v, p.y);
    w_f64(v, p.z);
}
fn w_f32_3(v: &mut Vec<u8>, a: [f32; 3]) {
    w_f32(v, a[0]);
    w_f32(v, a[1]);
    w_f32(v, a[2]);
}
fn w_f32_4(v: &mut Vec<u8>, a: [f32; 4]) {
    w_f32(v, a[0]);
    w_f32(v, a[1]);
    w_f32(v, a[2]);
    w_f32(v, a[3]);
}

struct R<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> R<'a> {
    fn u8(&mut self) -> Option<u8> {
        let x = *self.b.get(self.i)?;
        self.i += 1;
        Some(x)
    }
    fn u16(&mut self) -> Option<u16> {
        let s = self.b.get(self.i..self.i + 2)?;
        let x = u16::from_le_bytes(s.try_into().ok()?);
        self.i += 2;
        Some(x)
    }
    fn u32(&mut self) -> Option<u32> {
        let s = self.b.get(self.i..self.i + 4)?;
        let x = u32::from_le_bytes(s.try_into().ok()?);
        self.i += 4;
        Some(x)
    }
    fn u64(&mut self) -> Option<u64> {
        let s = self.b.get(self.i..self.i + 8)?;
        let x = u64::from_le_bytes(s.try_into().ok()?);
        self.i += 8;
        Some(x)
    }
    fn f32(&mut self) -> Option<f32> {
        let s = self.b.get(self.i..self.i + 4)?;
        let x = f32::from_le_bytes(s.try_into().ok()?);
        self.i += 4;
        Some(x)
    }
    fn f64(&mut self) -> Option<f64> {
        let s = self.b.get(self.i..self.i + 8)?;
        let x = f64::from_le_bytes(s.try_into().ok()?);
        self.i += 8;
        Some(x)
    }
    fn str(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        let s = self.b.get(self.i..self.i + n)?;
        self.i += n;
        Some(String::from_utf8(s.to_vec()).ok()?)
    }
    fn vec3d(&mut self) -> Option<Vec3d> {
        Some(Vec3d {
            x: self.f64()?,
            y: self.f64()?,
            z: self.f64()?,
        })
    }
    fn f32_3(&mut self) -> Option<[f32; 3]> {
        Some([self.f32()?, self.f32()?, self.f32()?])
    }
    fn f32_4(&mut self) -> Option<[f32; 4]> {
        Some([self.f32()?, self.f32()?, self.f32()?, self.f32()?])
    }
}

pub fn encode_c2s(m: &C2S) -> Vec<u8> {
    let mut v = Vec::with_capacity(160);
    match m {
        C2S::Hello { name, protocol } => {
            w_u8(&mut v, 0);
            w_str(&mut v, name);
            w_u32(&mut v, *protocol);
        }
        C2S::State {
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
            w_u8(&mut v, 1);
            w_vec3d(&mut v, *pos);
            w_f32_4(&mut v, *orient);
            w_f32(&mut v, *yaw);
            w_f32(&mut v, *cam_pitch);
            w_u8(&mut v, *boot);
            w_f32(&mut v, *throttle);
            w_u8(&mut v, *flags);
            w_f32(&mut v, *wing_arch);
            w_f32(&mut v, *wing_turn);
            w_f32(&mut v, *fall_spread);
            w_f32(&mut v, *fall_tuck);
            w_f32(&mut v, *strafe);
            w_f32(&mut v, *fwd_in);
            w_f32(&mut v, *jump_charge);
            w_f32(&mut v, *jump_push_timer);
            w_f32(&mut v, *landing_recovery);
            w_f32_3(&mut v, *rot_err);
            w_f32_3(&mut v, *intent_dir);
            w_f32_3(&mut v, *accel_smooth);
            w_f32_3(&mut v, *leg_steer);
            w_u8(&mut v, *pose_mode);
            w_f64(&mut v, *t);
        }
        C2S::Pose { seq, parts } => {
            w_u8(&mut v, 5);
            w_u32(&mut v, *seq);
            w_u16(&mut v, parts.len() as u16);
            for p in parts {
                w_u16(&mut v, p.index);
                w_f32_3(&mut v, p.pos);
                w_f32_4(&mut v, p.rot);
            }
        }
        C2S::AdminTime { unix_s } => {
            w_u8(&mut v, 3);
            w_f64(&mut v, *unix_s);
        }
        C2S::AdminWeather { preset } => {
            w_u8(&mut v, 4);
            w_str(&mut v, preset);
        }
        C2S::Bye => {
            w_u8(&mut v, 2);
        }
    }
    v
}

pub fn decode_c2s(b: &[u8]) -> Option<C2S> {
    let mut r = R { b, i: 0 };
    match r.u8()? {
        0 => Some(C2S::Hello {
            name: r.str()?,
            protocol: r.u32()?,
        }),
        1 => Some(C2S::State {
            pos: r.vec3d()?,
            orient: r.f32_4()?,
            yaw: r.f32()?,
            cam_pitch: r.f32()?,
            boot: r.u8()?,
            throttle: r.f32()?,
            flags: r.u8()?,
            wing_arch: r.f32()?,
            wing_turn: r.f32()?,
            fall_spread: r.f32()?,
            fall_tuck: r.f32()?,
            strafe: r.f32()?,
            fwd_in: r.f32()?,
            jump_charge: r.f32()?,
            jump_push_timer: r.f32()?,
            landing_recovery: r.f32()?,
            rot_err: r.f32_3()?,
            intent_dir: r.f32_3()?,
            accel_smooth: r.f32_3()?,
            leg_steer: r.f32_3()?,
            pose_mode: r.u8()?,
            t: r.f64()?,
        }),
        5 => {
            let seq = r.u32()?;
            let count = r.u16()? as usize;
            let mut parts = Vec::with_capacity(count);
            for _ in 0..count {
                parts.push(PosePart {
                    index: r.u16()?,
                    pos: r.f32_3()?,
                    rot: r.f32_4()?,
                });
            }
            Some(C2S::Pose { seq, parts })
        }
        3 => Some(C2S::AdminTime {
            unix_s: r.f64()?,
        }),
        4 => Some(C2S::AdminWeather {
            preset: r.str()?,
        }),
        2 => Some(C2S::Bye),
        _ => None,
    }
}

pub fn encode_s2c(m: &S2C) -> Vec<u8> {
    let mut v = Vec::with_capacity(180);
    match m {
        S2C::Welcome { id, players } => {
            w_u8(&mut v, 0);
            w_u64(&mut v, *id);
            w_u8(&mut v, players.len() as u8);
            for p in players {
                w_u64(&mut v, p.id);
                w_str(&mut v, &p.name);
            }
        }
        S2C::Join { id, name } => {
            w_u8(&mut v, 1);
            w_u64(&mut v, *id);
            w_str(&mut v, name);
        }
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
            w_u8(&mut v, 2);
            w_u64(&mut v, *id);
            w_str(&mut v, name);
            w_vec3d(&mut v, *pos);
            w_f32_4(&mut v, *orient);
            w_f32(&mut v, *yaw);
            w_f32(&mut v, *cam_pitch);
            w_u8(&mut v, *boot);
            w_f32(&mut v, *throttle);
            w_u8(&mut v, *flags);
            w_f32(&mut v, *wing_arch);
            w_f32(&mut v, *wing_turn);
            w_f32(&mut v, *fall_spread);
            w_f32(&mut v, *fall_tuck);
            w_f32(&mut v, *strafe);
            w_f32(&mut v, *fwd_in);
            w_f32(&mut v, *jump_charge);
            w_f32(&mut v, *jump_push_timer);
            w_f32(&mut v, *landing_recovery);
            w_f32_3(&mut v, *rot_err);
            w_f32_3(&mut v, *intent_dir);
            w_f32_3(&mut v, *accel_smooth);
            w_f32_3(&mut v, *leg_steer);
            w_u8(&mut v, *pose_mode);
            w_f64(&mut v, *t);
        }
        S2C::Pose { id, seq, parts } => {
            w_u8(&mut v, 6);
            w_u64(&mut v, *id);
            w_u32(&mut v, *seq);
            w_u16(&mut v, parts.len() as u16);
            for p in parts {
                w_u16(&mut v, p.index);
                w_f32_3(&mut v, p.pos);
                w_f32_4(&mut v, p.rot);
            }
        }
        S2C::Leave { id } => {
            w_u8(&mut v, 3);
            w_u64(&mut v, *id);
        }
        S2C::Time { unix_s, rate } => {
            w_u8(&mut v, 4);
            w_f64(&mut v, *unix_s);
            w_f64(&mut v, *rate);
        }
        S2C::Weather { preset } => {
            w_u8(&mut v, 5);
            w_str(&mut v, preset);
        }
    }
    v
}

pub fn decode_s2c(b: &[u8]) -> Option<S2C> {
    let mut r = R { b, i: 0 };
    match r.u8()? {
        0 => {
            let id = r.u64()?;
            let n = r.u8()? as usize;
            let mut players = Vec::new();
            for _ in 0..n {
                players.push(PlayerInfo {
                    id: r.u64()?,
                    name: r.str()?,
                })
            }
            Some(S2C::Welcome { id, players })
        }
        1 => Some(S2C::Join {
            id: r.u64()?,
            name: r.str()?,
        }),
        2 => Some(S2C::State {
            id: r.u64()?,
            name: r.str()?,
            pos: r.vec3d()?,
            orient: r.f32_4()?,
            yaw: r.f32()?,
            cam_pitch: r.f32()?,
            boot: r.u8()?,
            throttle: r.f32()?,
            flags: r.u8()?,
            wing_arch: r.f32()?,
            wing_turn: r.f32()?,
            fall_spread: r.f32()?,
            fall_tuck: r.f32()?,
            strafe: r.f32()?,
            fwd_in: r.f32()?,
            jump_charge: r.f32()?,
            jump_push_timer: r.f32()?,
            landing_recovery: r.f32()?,
            rot_err: r.f32_3()?,
            intent_dir: r.f32_3()?,
            accel_smooth: r.f32_3()?,
            leg_steer: r.f32_3()?,
            pose_mode: r.u8()?,
            t: r.f64()?,
        }),
        6 => {
            let id = r.u64()?;
            let seq = r.u32()?;
            let count = r.u16()? as usize;
            let mut parts = Vec::with_capacity(count);
            for _ in 0..count {
                parts.push(PosePart {
                    index: r.u16()?,
                    pos: r.f32_3()?,
                    rot: r.f32_4()?,
                });
            }
            Some(S2C::Pose { id, seq, parts })
        }
        3 => Some(S2C::Leave { id: r.u64()? }),
        4 => Some(S2C::Time {
            unix_s: r.f64()?,
            rate: r.f64()?,
        }),
        5 => Some(S2C::Weather {
            preset: r.str()?,
        }),
        _ => None,
    }
}

// legacy generic wrappers (unused, kept for compat)
pub fn encode<T: serde::Serialize>(_v: &T) -> Vec<u8> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c2s_state_roundtrip() {
        let msg = C2S::State {
            pos: Vec3d { x: 123456.78, y: -987654.32, z: 456789.01 },
            orient: [0.1, 0.2, 0.3, 0.9],
            yaw: 1.25,
            cam_pitch: -0.34,
            boot: 2,
            throttle: 0.85,
            flags: 7,
            wing_arch: 0.45,
            wing_turn: -0.12,
            fall_spread: 0.88,
            fall_tuck: 0.05,
            strafe: -0.5,
            fwd_in: 1.0,
            jump_charge: 0.75,
            jump_push_timer: 0.2,
            landing_recovery: 0.1,
            rot_err: [0.01, -0.02, 0.03],
            intent_dir: [0.5, 0.0, -0.86],
            accel_smooth: [0.1, 9.8, 0.2],
            leg_steer: [0.02, -0.01, 0.04],
            pose_mode: 0,
            t: 1794038034.5,
        };
        let encoded = encode_c2s(&msg);
        let decoded = decode_c2s(&encoded).expect("decode failed");
        match decoded {
            C2S::State { pos, orient, yaw, boot, throttle, flags, rot_err, intent_dir, t, .. } => {
                assert_eq!(pos, Vec3d { x: 123456.78, y: -987654.32, z: 456789.01 });
                assert_eq!(orient, [0.1, 0.2, 0.3, 0.9]);
                assert_eq!(yaw, 1.25);
                assert_eq!(boot, 2);
                assert_eq!(throttle, 0.85);
                assert_eq!(flags, 7);
                assert_eq!(rot_err, [0.01, -0.02, 0.03]);
                assert_eq!(intent_dir, [0.5, 0.0, -0.86]);
                assert_eq!(t, 1794038034.5);
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn pose_roundtrip() {
        let parts = vec![
            PosePart { index: 0, pos: [0.1, 0.2, 0.3], rot: [0.0, 0.0, 0.0, 1.0] },
            PosePart { index: 5, pos: [-0.4, 0.5, -0.6], rot: [0.5, 0.5, 0.5, 0.5] },
        ];
        let c2s_pose = C2S::Pose { seq: 42, parts: parts.clone() };
        let enc_c2s = encode_c2s(&c2s_pose);
        match decode_c2s(&enc_c2s).unwrap() {
            C2S::Pose { seq, parts: p } => {
                assert_eq!(seq, 42);
                assert_eq!(p, parts);
            }
            _ => panic!(),
        }

        let s2c_pose = S2C::Pose { id: 99, seq: 43, parts: parts.clone() };
        let enc_s2c = encode_s2c(&s2c_pose);
        match decode_s2c(&enc_s2c).unwrap() {
            S2C::Pose { id, seq, parts: p } => {
                assert_eq!(id, 99);
                assert_eq!(seq, 43);
                assert_eq!(p, parts);
            }
            _ => panic!(),
        }
    }
}
