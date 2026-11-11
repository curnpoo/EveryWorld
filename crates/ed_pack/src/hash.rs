//! Content hashes and asset ids (both BLAKE3, 32 bytes).

use std::fmt;
use std::str::FromStr;

/// BLAKE3 of the encoded chunk bytes (header + body). Equal hash means equal chunk.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ChunkHash(pub [u8; 32]);

impl ChunkHash {
    pub fn of(bytes: &[u8]) -> ChunkHash {
        ChunkHash(*blake3::hash(bytes).as_bytes())
    }
}

/// BLAKE3 of the source asset path. The caller normalises the path (forward slashes,
/// relative to the asset root) before hashing.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AssetId(pub [u8; 32]);

impl AssetId {
    pub fn from_path(path: &str) -> AssetId {
        AssetId(*blake3::hash(path.as_bytes()).as_bytes())
    }
}

pub(crate) fn hex(b: &[u8]) -> String {
    const D: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push(D[(x >> 4) as usize] as char);
        s.push(D[(x & 15) as usize] as char);
    }
    s
}

fn unhex32(s: &str) -> Result<[u8; 32], ()> {
    let b = s.as_bytes();
    if b.len() != 64 {
        return Err(());
    }
    let nib = |c: u8| match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(()),
    };
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = nib(b[2 * i])? << 4 | nib(b[2 * i + 1])?;
    }
    Ok(out)
}

macro_rules! hash_impls {
    ($t:ident) => {
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&hex(&self.0))
            }
        }
        impl fmt::Debug for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($t), &hex(&self.0)[..16])
            }
        }
        impl FromStr for $t {
            type Err = ();
            fn from_str(s: &str) -> Result<$t, ()> {
                unhex32(s).map($t)
            }
        }
    };
}
hash_impls!(ChunkHash);
hash_impls!(AssetId);
