//! P3: a 200 MB pack; opening it and reading one chunk touches only the header, the manifest
//! and that chunk. Prints write/read throughput (run with `--nocapture`).

mod common;

use std::fs::File;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use common::*;
use ed_pack::*;

/// Counts every byte range read through it.
struct Counting {
    inner: File,
    bytes: AtomicU64,
    reads: std::sync::Mutex<Vec<(u64, usize)>>,
}

impl ReadAt for Counting {
    fn size(&self) -> io::Result<u64> {
        self.inner.size()
    }
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        self.bytes.fetch_add(buf.len() as u64, Ordering::Relaxed);
        self.reads.lock().unwrap().push((offset, buf.len()));
        self.inner.read_exact_at(buf, offset)
    }
}

const CHUNK: usize = 1 << 20; // 1 MiB payloads
const COUNT: u32 = 200;

#[test]
fn two_hundred_mb_pack_random_access() {
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ed_pack_big_test.ewpack");
    let mut rng = Rng(0xB16);
    let mut w = PackWriter::new(1, 1);
    let mut keys = Vec::new();
    let mut payload = vec![0u8; CHUNK];
    let t = Instant::now();
    for i in 0..COUNT {
        rng.fill(&mut payload);
        let key = ChunkKey::node(earth_node(14, 8000 + i % 20, 9000 + i / 20), ChunkKind::Terrain, Rung::R3);
        w.insert(key, Chunk::build(ChunkKind::Terrain, &meta(), &payload).unwrap()).unwrap();
        keys.push(key);
    }
    let build_s = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let s = w.write_file(&path).unwrap();
    let write_s = t.elapsed().as_secs_f64();
    drop(w);
    let mb = s.bytes as f64 / 1e6;
    assert!(mb >= 200.0, "{mb} MB");

    // Open + one read through a counting reader.
    let counting = Counting { inner: File::open(&path).unwrap(), bytes: AtomicU64::new(0), reads: Default::default() };
    let t = Instant::now();
    let r = PackReader::open(counting).unwrap();
    let open_s = t.elapsed().as_secs_f64();
    let after_open = r.storage().bytes.load(Ordering::Relaxed);
    let manifest_len = s.bytes - r.manifest_offset();
    assert!(after_open <= 64 + manifest_len, "open read {after_open} B, manifest is {manifest_len} B");
    let target = keys[137];
    let e = *r.entry(&target).unwrap();
    let c = r.read(&target).unwrap();
    assert_eq!(c.payload().len(), CHUNK);
    let total = r.storage().bytes.load(Ordering::Relaxed);
    assert_eq!(total - after_open, e.len as u64, "one read touched only that chunk");
    let reads = r.storage().reads.lock().unwrap().clone();
    for (off, len) in &reads {
        let in_header = *off + *len as u64 <= 64;
        let in_manifest = *off >= r.manifest_offset();
        let in_chunk = *off >= e.offset && *off + *len as u64 <= e.offset + e.len as u64;
        assert!(in_header || in_manifest || in_chunk, "stray read at {off}+{len}");
    }
    println!(
        "big pack: {mb:.1} MB, {} reads, {} B touched for open+1 chunk ({:.3}% of file); open {:.2} ms",
        reads.len(),
        total,
        100.0 * total as f64 / s.bytes as f64,
        open_s * 1e3
    );

    // Throughput: full verified sequential read through the map.
    let src = PackFileSource::open(&path).unwrap();
    let t = Instant::now();
    let mut n = 0u64;
    for k in &keys {
        n += src.get(k).unwrap().bytes().len() as u64;
    }
    let read_s = t.elapsed().as_secs_f64();
    let t = Instant::now();
    src.reader().verify_all().unwrap();
    let verify_s = t.elapsed().as_secs_f64();
    println!(
        "big pack: build {:.0} MB/s (encode+hash), write {:.0} MB/s, read+verify {:.0} MB/s (copying), verify_all {:.0} MB/s (zero-copy)",
        mb / build_s,
        mb / write_s,
        n as f64 / 1e6 / read_s,
        mb / verify_s
    );
    drop(src);
    drop(r);
    std::fs::remove_file(&path).unwrap();
}
