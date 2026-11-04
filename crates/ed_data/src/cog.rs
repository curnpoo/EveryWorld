//! A `Read + Seek` view of a remote file built from HTTP range requests, with
//! every 256 KB block cached on disk forever. Feeding it to `tiff::Decoder`
//! turns a multi-hundred-MB Cloud-Optimized GeoTIFF into "download only the
//! header and the few tiles actually looked at".

use std::collections::HashMap;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Arc;

use crate::cache::DiskCache;
use crate::http;

const BLOCK: u64 = 256 * 1024;
const MAX_MEM_BLOCKS: usize = 128;

pub struct RangeReader {
    url: String,
    key: String,
    cache: DiskCache,
    total: u64,
    pos: u64,
    blocks: HashMap<u64, Arc<Vec<u8>>>,
}

impl RangeReader {
    /// `Ok(None)` when the object does not exist (e.g. an all-ocean tile).
    pub fn open(url: &str, key: &str, cache: &DiskCache) -> anyhow::Result<Option<Self>> {
        let len_key = format!("{key}.len");
        let total = match cache.get(&len_key, None).and_then(|b| String::from_utf8(b).ok()).and_then(|s| s.trim().parse::<u64>().ok()) {
            Some(t) => t,
            None => {
                let Some((bytes, total)) = http::get_range(url, 0, BLOCK)? else { return Ok(None) };
                let _ = cache.put(&format!("{key}.0"), &bytes);
                let _ = cache.put(&len_key, total.to_string().as_bytes());
                total
            }
        };
        Ok(Some(Self { url: url.into(), key: key.into(), cache: cache.clone(), total, pos: 0, blocks: HashMap::new() }))
    }

    fn block(&mut self, i: u64) -> io::Result<Arc<Vec<u8>>> {
        if let Some(b) = self.blocks.get(&i) {
            return Ok(b.clone());
        }
        let ck = format!("{}.{}", self.key, i);
        let start = i * BLOCK;
        let want = BLOCK.min(self.total - start) as usize;
        let data = match self.cache.get(&ck, None).filter(|d| d.len() == want) {
            Some(d) => d,
            None => {
                let (d, _) = http::get_range(&self.url, start, want as u64)
                    .map_err(|e| io::Error::other(e.to_string()))?
                    .ok_or_else(|| io::Error::other("range 404"))?;
                if d.len() != want {
                    return Err(io::Error::other(format!("short range read {} != {want}", d.len())));
                }
                let _ = self.cache.put(&ck, &d);
                d
            }
        };
        if self.blocks.len() >= MAX_MEM_BLOCKS {
            self.blocks.clear();
        }
        let a = Arc::new(data);
        self.blocks.insert(i, a.clone());
        Ok(a)
    }
}

impl Read for RangeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.total || buf.is_empty() {
            return Ok(0);
        }
        let i = self.pos / BLOCK;
        let b = self.block(i)?;
        let off = (self.pos - i * BLOCK) as usize;
        let n = buf.len().min(b.len() - off);
        buf[..n].copy_from_slice(&b[off..off + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for RangeReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let np = match from {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::End(d) => self.total as i64 + d,
            SeekFrom::Current(d) => self.pos as i64 + d,
        };
        if np < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before start"));
        }
        self.pos = np as u64;
        Ok(self.pos)
    }
}
