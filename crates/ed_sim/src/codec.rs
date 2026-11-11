//! Little-endian byte codec for snapshots and replays. Floats are written as their exact bits.

use crate::SimError;
use glam::{DQuat, DVec3, Quat, Vec3};

#[derive(Default)]
pub(crate) struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    pub fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }
    pub fn len(&mut self, n: usize) {
        self.u32(u32::try_from(n).expect("collection too large"));
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.len(b.len());
        self.buf.extend_from_slice(b);
    }
    pub fn vec3(&mut self, v: Vec3) {
        self.f32(v.x);
        self.f32(v.y);
        self.f32(v.z);
    }
    pub fn dvec3(&mut self, v: DVec3) {
        self.f64(v.x);
        self.f64(v.y);
        self.f64(v.z);
    }
    pub fn quat(&mut self, q: Quat) {
        self.f32(q.x);
        self.f32(q.y);
        self.f32(q.z);
        self.f32(q.w);
    }
    pub fn dquat(&mut self, q: DQuat) {
        self.f64(q.x);
        self.f64(q.y);
        self.f64(q.z);
        self.f64(q.w);
    }
}

pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

type R<T> = Result<T, SimError>;

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }
    pub fn done(&self) -> bool {
        self.pos == self.data.len()
    }
    fn take(&mut self, n: usize) -> R<&'a [u8]> {
        if self.data.len() - self.pos < n {
            return Err(SimError::Corrupt("truncated"));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    pub fn u8(&mut self) -> R<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> R<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(SimError::Corrupt("bad bool")),
        }
    }
    pub fn u32(&mut self) -> R<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> R<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn f32(&mut self) -> R<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn f64(&mut self) -> R<f64> {
        Ok(f64::from_bits(self.u64()?))
    }
    /// A collection length, sanity-checked against the bytes left (each element is at least
    /// `min_elem` bytes) so a corrupt length cannot trigger a huge allocation.
    pub fn len(&mut self, min_elem: usize) -> R<usize> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min_elem.max(1)) > self.data.len() - self.pos {
            return Err(SimError::Corrupt("length out of range"));
        }
        Ok(n)
    }
    pub fn bytes(&mut self) -> R<&'a [u8]> {
        let n = self.len(1)?;
        self.take(n)
    }
    pub fn vec3(&mut self) -> R<Vec3> {
        Ok(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }
    pub fn dvec3(&mut self) -> R<DVec3> {
        Ok(DVec3::new(self.f64()?, self.f64()?, self.f64()?))
    }
    pub fn quat(&mut self) -> R<Quat> {
        Ok(Quat::from_xyzw(self.f32()?, self.f32()?, self.f32()?, self.f32()?))
    }
    pub fn dquat(&mut self) -> R<DQuat> {
        Ok(DQuat::from_xyzw(self.f64()?, self.f64()?, self.f64()?, self.f64()?))
    }
}

/// FNV-1a 64 over explicit little-endian bytes: the same on every platform.
#[derive(Clone, Copy)]
pub(crate) struct Fnv(pub u64);

impl Fnv {
    pub const fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    #[inline]
    pub fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = (self.0 ^ x as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    #[inline]
    pub fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    #[inline]
    pub fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    #[inline]
    pub fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    #[inline]
    pub fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }
}
