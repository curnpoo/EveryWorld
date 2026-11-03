//! Minimal 16-bit PCM WAV read/write (renders, tests, the reference clips).

use std::io::{self, Read, Write};
use std::path::Path;

/// Write interleaved stereo (or mono when `r` is None) 16-bit PCM, rounding
/// and clipping like the kit's `render.mjs`.
pub fn write(path: &Path, sr: u32, l: &[f32], r: Option<&[f32]>) -> io::Result<()> {
    let ch: u16 = if r.is_some() { 2 } else { 1 };
    let n = l.len();
    let data_len = (n * ch as usize * 2) as u32;
    let mut buf = Vec::with_capacity(44 + data_len as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_len).to_le_bytes());
    buf.extend_from_slice(b"WAVEfmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&ch.to_le_bytes());
    buf.extend_from_slice(&sr.to_le_bytes());
    buf.extend_from_slice(&(sr * ch as u32 * 2).to_le_bytes());
    buf.extend_from_slice(&(ch * 2).to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    let q = |x: f32| -> i16 { ((x as f64) * 32767.0).round().clamp(-32767.0, 32767.0) as i16 };
    for i in 0..n {
        buf.extend_from_slice(&q(l[i]).to_le_bytes());
        if let Some(r) = r {
            buf.extend_from_slice(&q(r[i]).to_le_bytes());
        }
    }
    std::fs::File::create(path)?.write_all(&buf)
}

/// A decoded 16-bit PCM WAV: sample rate and one Vec per channel (-1..1).
pub struct Wav {
    pub sample_rate: u32,
    pub channels: Vec<Vec<f32>>,
}

pub fn read(path: &Path) -> io::Result<Wav> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.read_to_end(&mut bytes)?;
    parse(&bytes)
}

pub fn parse(bytes: &[u8]) -> io::Result<Wav> {
    let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_string());
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(bad("not a WAV"));
    }
    let (mut pos, mut ch, mut sr, mut bits) = (12usize, 0u16, 0u32, 0u16);
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body = &bytes[pos + 8..(pos + 8 + len).min(bytes.len())];
        if id == b"fmt " {
            ch = u16::from_le_bytes(body[2..4].try_into().unwrap());
            sr = u32::from_le_bytes(body[4..8].try_into().unwrap());
            bits = u16::from_le_bytes(body[14..16].try_into().unwrap());
        } else if id == b"data" {
            if bits != 16 || ch == 0 {
                return Err(bad("only 16-bit PCM"));
            }
            let frames = body.len() / (2 * ch as usize);
            let mut channels = vec![Vec::with_capacity(frames); ch as usize];
            for f in 0..frames {
                for (c, chan) in channels.iter_mut().enumerate() {
                    let o = (f * ch as usize + c) * 2;
                    chan.push(i16::from_le_bytes([body[o], body[o + 1]]) as f32 / 32767.0);
                }
            }
            return Ok(Wav { sample_rate: sr, channels });
        }
        pos += 8 + len + (len & 1);
    }
    Err(bad("no data chunk"))
}
