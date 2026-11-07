//! Multiplayer wire protocol (binary, compact).
//!
//! Engine-free. Compact binary encoding (no JSON): ~40 bytes per State.

pub const PROTOCOL_VERSION: u32 = 4;
pub const DEFAULT_PORT: u16 = 7777;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3d { pub x: f64, pub y: f64, pub z: f64 }
impl From<[f64;3]> for Vec3d { fn from(a:[f64;3])->Self{Self{x:a[0],y:a[1],z:a[2]}} }
impl From<Vec3d> for [f64;3] { fn from(v:Vec3d)->Self{[v.x,v.y,v.z]} }

#[derive(Debug, Clone)]
pub enum C2S {
    Hello { name: String, protocol: u32 },
    State { pos: Vec3d, yaw: f32, pitch: f32, boot: u8, throttle: f32, wingsuit: bool, t: f64 },
    AdminTime { unix_s: f64 },
    AdminWeather { preset: String },
    Bye,
}

#[derive(Debug, Clone)]
pub enum S2C {
    Welcome { id: u64, players: Vec<PlayerInfo> },
    Join { id: u64, name: String },
    State { id: u64, name: String, pos: Vec3d, yaw: f32, pitch: f32, boot: u8, throttle: f32, wingsuit: bool, t: f64 },
    Leave { id: u64 },
    Time { unix_s: f64, rate: f64 },
    Weather { preset: String },
}

#[derive(Debug, Clone)]
pub struct PlayerInfo { pub id: u64, pub name: String }

// ---- binary helpers ----
fn w_u8(v:&mut Vec<u8>, x:u8){v.push(x)}
fn w_u32(v:&mut Vec<u8>, x:u32){v.extend_from_slice(&x.to_le_bytes())}
fn w_u64(v:&mut Vec<u8>, x:u64){v.extend_from_slice(&x.to_le_bytes())}
fn w_f32(v:&mut Vec<u8>, x:f32){v.extend_from_slice(&x.to_le_bytes())}
fn w_f64(v:&mut Vec<u8>, x:f64){v.extend_from_slice(&x.to_le_bytes())}
fn w_str(v:&mut Vec<u8>, s:&str){v.push(s.len() as u8); v.extend_from_slice(s.as_bytes())}
struct R<'a>{b:&'a [u8],i:usize}
impl<'a> R<'a>{
    fn u8(&mut self)->Option<u8>{let x=*self.b.get(self.i)?;self.i+=1;Some(x)}
    fn u32(&mut self)->Option<u32>{let s=self.b.get(self.i..self.i+4)?;let x=u32::from_le_bytes(s.try_into().ok()?);self.i+=4;Some(x)}
    fn u64(&mut self)->Option<u64>{let s=self.b.get(self.i..self.i+8)?;let x=u64::from_le_bytes(s.try_into().ok()?);self.i+=8;Some(x)}
    fn f32(&mut self)->Option<f32>{let s=self.b.get(self.i..self.i+4)?;let x=f32::from_le_bytes(s.try_into().ok()?);self.i+=4;Some(x)}
    fn f64(&mut self)->Option<f64>{let s=self.b.get(self.i..self.i+8)?;let x=f64::from_le_bytes(s.try_into().ok()?);self.i+=8;Some(x)}
    fn str(&mut self)->Option<String>{let n=self.u8()? as usize;let s=self.b.get(self.i..self.i+n)?;self.i+=n;Some(String::from_utf8(s.to_vec()).ok()?)}
    fn vec3d(&mut self)->Option<Vec3d>{Some(Vec3d{x:self.f64()?,y:self.f64()?,z:self.f64()?})}
}
fn w_vec3d(v:&mut Vec<u8>, p:Vec3d){w_f64(v,p.x);w_f64(v,p.y);w_f64(v,p.z)}

pub fn encode_c2s(m:&C2S)->Vec<u8>{
    let mut v=Vec::with_capacity(48);
    match m{
        C2S::Hello{name,protocol}=>{w_u8(&mut v,0);w_str(&mut v,name);w_u32(&mut v,*protocol)}
        C2S::State{pos,yaw,pitch,boot,throttle,wingsuit,t}=>{w_u8(&mut v,1);w_vec3d(&mut v,*pos);w_f32(&mut v,*yaw);w_f32(&mut v,*pitch);w_u8(&mut v,*boot);w_f32(&mut v,*throttle);w_u8(&mut v,if *wingsuit{1}else{0});w_f64(&mut v,*t)}
        C2S::AdminTime{unix_s}=>{w_u8(&mut v,3);w_f64(&mut v,*unix_s)}
        C2S::AdminWeather{preset}=>{w_u8(&mut v,4);w_str(&mut v,preset)}
        C2S::Bye=>{w_u8(&mut v,2)}
    }
    v
}
pub fn decode_c2s(b:&[u8])->Option<C2S>{
    let mut r=R{b,i:0};
    match r.u8()?{
        0=>Some(C2S::Hello{name:r.str()?,protocol:r.u32()?}),
        1=>Some(C2S::State{pos:r.vec3d()?,yaw:r.f32()?,pitch:r.f32()?,boot:r.u8()?,throttle:r.f32()?,wingsuit:r.u8()?!=0,t:r.f64()?}),
        3=>Some(C2S::AdminTime{unix_s:r.f64()?}),
        4=>Some(C2S::AdminWeather{preset:r.str()?}),
        2=>Some(C2S::Bye),
        _=>None,
    }
}
pub fn encode_s2c(m:&S2C)->Vec<u8>{
    let mut v=Vec::with_capacity(56);
    match m{
        S2C::Welcome{id,players}=>{w_u8(&mut v,0);w_u64(&mut v,*id);w_u8(&mut v,players.len() as u8);for p in players{w_u64(&mut v,p.id);w_str(&mut v,&p.name)}}
        S2C::Join{id,name}=>{w_u8(&mut v,1);w_u64(&mut v,*id);w_str(&mut v,name)}
        S2C::State{id,name,pos,yaw,pitch,boot,throttle,wingsuit,t}=>{w_u8(&mut v,2);w_u64(&mut v,*id);w_str(&mut v,name);w_vec3d(&mut v,*pos);w_f32(&mut v,*yaw);w_f32(&mut v,*pitch);w_u8(&mut v,*boot);w_f32(&mut v,*throttle);w_u8(&mut v,if *wingsuit{1}else{0});w_f64(&mut v,*t)}
        S2C::Leave{id}=>{w_u8(&mut v,3);w_u64(&mut v,*id)}
        S2C::Time{unix_s,rate}=>{w_u8(&mut v,4);w_f64(&mut v,*unix_s);w_f64(&mut v,*rate)}
        S2C::Weather{preset}=>{w_u8(&mut v,5);w_str(&mut v,preset)}
    }
    v
}
pub fn decode_s2c(b:&[u8])->Option<S2C>{
    let mut r=R{b,i:0};
    match r.u8()?{
        0=>{let id=r.u64()?;let n=r.u8()? as usize;let mut players=Vec::new();for _ in 0..n{players.push(PlayerInfo{id:r.u64()?,name:r.str()?})}Some(S2C::Welcome{id,players})}
        1=>Some(S2C::Join{id:r.u64()?,name:r.str()?}),
        2=>Some(S2C::State{id:r.u64()?,name:r.str()?,pos:r.vec3d()?,yaw:r.f32()?,pitch:r.f32()?,boot:r.u8()?,throttle:r.f32()?,wingsuit:r.u8()?!=0,t:r.f64()?}),
        3=>Some(S2C::Leave{id:r.u64()?}),
        4=>Some(S2C::Time{unix_s:r.f64()?,rate:r.f64()?}),
        5=>Some(S2C::Weather{preset:r.str()?}),
        _=>None,
    }
}
// legacy generic wrappers (unused, kept for compat)
pub fn encode<T: serde::Serialize>(_v:&T)->Vec<u8>{Vec::new()}
