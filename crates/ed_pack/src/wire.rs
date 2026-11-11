//! Little-endian byte writer and bounds-checked reader shared by every chunk body.
//!
//! Every read is checked; corrupt input produces [`Error`], never a panic. Counts read from
//! the input are checked against the remaining bytes before anything is allocated.

use crate::error::{Error, Result};

/// Alignment of chunks in a pack, of chunk payloads, and of bulk streams inside bodies.
pub const ALIGN: usize = 16;

pub(crate) fn pad_len(n: usize) -> usize {
    (ALIGN - n % ALIGN) % ALIGN
}

pub(crate) struct Out<'v> {
    pub buf: &'v mut Vec<u8>,
}

impl<'v> Out<'v> {
    pub fn new(buf: &'v mut Vec<u8>) -> Self {
        Out { buf }
    }
    pub fn pos(&self) -> usize {
        self.buf.len()
    }
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u128(&mut self, v: u128) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f32s(&mut self, v: &[f32]) {
        for x in v {
            self.f32(*x);
        }
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
    /// A `u32` element count.
    pub fn count(&mut self, n: usize, what: &str) -> Result<()> {
        let n = u32::try_from(n).map_err(|_| Error::Invalid(format!("too many {what}: {n}")))?;
        self.u32(n);
        Ok(())
    }
    /// `u16` length then UTF-8 bytes.
    pub fn str16(&mut self, s: &str) -> Result<()> {
        let n = u16::try_from(s.len())
            .map_err(|_| Error::Invalid(format!("string longer than 65535 bytes: {}...", &s[..16.min(s.len())])))?;
        self.u16(n);
        self.bytes(s.as_bytes());
        Ok(())
    }
    /// Zero padding up to the next multiple of [`ALIGN`] from the start of the buffer.
    pub fn align(&mut self) {
        let n = pad_len(self.buf.len());
        self.buf.resize(self.buf.len() + n, 0);
    }
    pub fn patch_u32(&mut self, at: usize, v: u32) {
        self.buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    pub fn patch_u64(&mut self, at: usize, v: u64) {
        self.buf[at..at + 8].copy_from_slice(&v.to_le_bytes());
    }
}

pub(crate) struct In<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> In<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        In { b, pos: 0 }
    }
    pub fn pos(&self) -> usize {
        self.pos
    }
    pub fn remaining(&self) -> usize {
        self.b.len() - self.pos
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.remaining() {
            return Err(Error::Truncated { offset: self.pos, needed: n, available: self.remaining() });
        }
        let s = &self.b[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    pub fn arr<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.take(N)?);
        Ok(a)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.arr::<1>()?[0])
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.arr()?))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.arr()?))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.arr()?))
    }
    pub fn u128(&mut self) -> Result<u128> {
        Ok(u128::from_le_bytes(self.arr()?))
    }
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.arr()?))
    }
    pub fn f32x<const N: usize>(&mut self) -> Result<[f32; N]> {
        let mut a = [0f32; N];
        for x in &mut a {
            *x = self.f32()?;
        }
        Ok(a)
    }
    /// A `u32` count whose elements take at least `min_elem` bytes each; rejects counts the
    /// remaining input cannot hold, so corrupt counts never cause large allocations.
    pub fn count(&mut self, min_elem: usize) -> Result<usize> {
        let at = self.pos;
        let n = self.u32()? as usize;
        let need = n.saturating_mul(min_elem.max(1));
        if need > self.remaining() {
            return Err(Error::Truncated { offset: at, needed: need, available: self.remaining() });
        }
        Ok(n)
    }
    pub fn str16(&mut self) -> Result<&'a str> {
        let n = self.u16()? as usize;
        let at = self.pos;
        std::str::from_utf8(self.take(n)?).map_err(|_| Error::Corrupt(format!("invalid UTF-8 at byte {at}")))
    }
    /// Skip padding to the next multiple of [`ALIGN`]; padding must be zero.
    pub fn align(&mut self) -> Result<()> {
        let at = self.pos;
        let pad = self.take(pad_len(self.pos))?;
        if pad.iter().any(|b| *b != 0) {
            return Err(Error::Corrupt(format!("non-zero padding at byte {at}")));
        }
        Ok(())
    }
    pub fn zeros(&mut self, n: usize, what: &str) -> Result<()> {
        let at = self.pos;
        if self.take(n)?.iter().any(|b| *b != 0) {
            return Err(Error::Corrupt(format!("{what} at byte {at} must be zero")));
        }
        Ok(())
    }
    pub fn finish(&self) -> Result<()> {
        if self.pos != self.b.len() {
            return Err(Error::Corrupt(format!("{} trailing bytes after byte {}", self.remaining(), self.pos)));
        }
        Ok(())
    }
}
