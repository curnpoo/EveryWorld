//! Pack container: determinism (P6), dedupe and lookup (P4), corruption (P2), patch mounts
//! (P5), licence manifest, golden hash, fuzzing.

mod common;

use std::borrow::Cow;

use common::*;
use ed_geo::NodeId;
use ed_pack::kinds::*;
use ed_pack::*;

/// A few hundred `(key, chunk)` pairs: node chunks over a small quadtree, assets, layers.
fn corpus() -> Vec<(ChunkKey, Chunk<'static>)> {
    let mut v = Vec::new();
    let mut rng = Rng(42);
    for x in 0..8u32 {
        for y in 0..8u32 {
            let n = earth_node(12, 1000 + x, 2000 + y);
            let mut bytes = vec![0u8; 64 + rng.below(512)];
            rng.fill(&mut bytes);
            v.push((ChunkKey::node(n, ChunkKind::Terrain, Rung::R3), opaque(ChunkKind::Terrain, bytes)));
            v.push((
                ChunkKey::node(n, ChunkKind::Instances, Rung::R0),
                Chunk::encode(&sample_instances(1 + (x * 8 + y) as usize % 7, (x * 8 + y) as u64), &meta()).unwrap(),
            ));
            // Ocean-like duplicate content under many keys.
            v.push((ChunkKey::node(n, ChunkKind::Ground, Rung::R0), opaque(ChunkKind::Ground, vec![7u8; 300])));
            v.push((ChunkKey::node(n, ChunkKind::Collision, Rung::R0), Chunk::encode(&sample_collision(false), &meta()).unwrap()));
        }
    }
    for i in 0..40 {
        let a = asset(&format!("mesh/m{i}"));
        let mut m = sample_mesh();
        m.bounds.radius = 1.0 + i as f32;
        v.push((ChunkKey::asset(a, ChunkKind::Mesh), Chunk::encode(&m, &meta()).unwrap()));
        let mut mat = sample_material();
        mat.params.insert("id".into(), Param::Float(i as f32));
        v.push((ChunkKey::asset(asset(&format!("mat/m{i}")), ChunkKind::Material), Chunk::encode(&mat, &meta()).unwrap()));
    }
    v.push((ChunkKey::asset(asset("tex/a"), ChunkKind::Texture), Chunk::encode(&sample_texture(), &meta()).unwrap()));
    v.push((ChunkKey::asset(asset("prefab/hall"), ChunkKind::Prefab), Chunk::encode(&sample_prefab(), &meta()).unwrap()));
    let l10 = earth_node(10, 250, 500);
    v.push((ChunkKey::layer(l10, "base_hubs"), Chunk::encode(&LayerText { text: Cow::Borrowed("") }, &meta()).unwrap()));
    v
}

fn build(pairs: &[(ChunkKey, Chunk<'static>)]) -> (Vec<u8>, PackSummary) {
    let mut w = PackWriter::new(3, 7);
    for (k, c) in pairs {
        w.insert(*k, c.clone()).unwrap();
    }
    w.to_bytes().unwrap()
}

#[test]
fn identical_bytes_for_any_insertion_order() {
    let pairs = corpus();
    assert!(pairs.len() >= 300, "{} pairs", pairs.len());
    let (a, sa) = build(&pairs);
    let mut rev = pairs.clone();
    rev.reverse();
    let (b, sb) = build(&rev);
    let mut shuffled = pairs.clone();
    Rng(7).shuffle(&mut shuffled);
    let (c, sc) = build(&shuffled);
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_eq!(sa.file_hash, sb.file_hash);
    assert_eq!(sa.file_hash, sc.file_hash);
    assert_eq!(sa.file_hash, ChunkHash::of(&a));
    // Inserting from several threads (then merging in any order) gives the same file too.
    let halves: Vec<Vec<(ChunkKey, Chunk<'static>)>> = std::thread::scope(|s| {
        let h: Vec<_> = shuffled.chunks(shuffled.len() / 4 + 1).map(|part| s.spawn(move || part.to_vec())).collect();
        h.into_iter().rev().map(|j| j.join().unwrap()).collect()
    });
    let (d, _) = build(&halves.concat());
    assert_eq!(a, d);
}

#[test]
fn duplicate_content_is_stored_once() {
    let pairs = corpus();
    let (bytes, s) = build(&pairs);
    let r = PackReader::open(bytes).unwrap();
    assert_eq!(r.key_count(), pairs.len());
    // 64 Ground keys share one chunk, 64 Collision keys share one chunk.
    let distinct: std::collections::BTreeSet<_> = pairs.iter().map(|(_, c)| c.hash()).collect();
    assert_eq!(s.chunks, distinct.len());
    assert_eq!(r.chunk_count(), distinct.len());
    assert!(distinct.len() <= pairs.len() - 126);
    let ground: Vec<_> = r.entries().iter().filter(|e| e.key.kind() == ChunkKind::Ground).collect();
    assert_eq!(ground.len(), 64);
    assert!(ground.iter().all(|e| e.offset == ground[0].offset));
    r.verify_all().unwrap();
}

#[test]
fn same_key_twice_needs_the_same_chunk() {
    let mut w = PackWriter::new(0, 0);
    let k = ChunkKey::node(earth_node(12, 1, 1), ChunkKind::Terrain, Rung::R3);
    w.insert(k, opaque(ChunkKind::Terrain, vec![1])).unwrap();
    w.insert(k, opaque(ChunkKind::Terrain, vec![1])).unwrap();
    assert!(matches!(w.insert(k, opaque(ChunkKind::Terrain, vec![2])), Err(Error::DuplicateKey { .. })));
    // Key kind must match chunk kind; node keys need node kinds.
    assert!(matches!(w.insert(k, opaque(ChunkKind::Nav, vec![1])), Err(Error::KeyKindMismatch { .. })));
    let bad = ChunkKey::node(earth_node(12, 1, 1), ChunkKind::Mesh, Rung::R0);
    assert!(w.insert(bad, Chunk::encode(&sample_mesh(), &meta()).unwrap()).is_err());
}

#[test]
fn lookup_by_node_kind_rung_and_listings() {
    let pairs = corpus();
    let (bytes, _) = build(&pairs);
    let r = PackReader::open(bytes).unwrap();
    for (k, c) in &pairs {
        assert_eq!(r.resolve(k), Some(c.hash()), "{k:?}");
        let got = r.read(k).unwrap();
        assert_eq!(got.bytes(), c.bytes());
    }
    let n = earth_node(12, 1003, 2004);
    assert!(r.resolve(&ChunkKey::node(n, ChunkKind::Terrain, Rung::R4)).is_none());
    assert!(matches!(r.read(&ChunkKey::node(n, ChunkKind::Nav, Rung::R0)), Err(Error::KeyNotFound(_))));

    // All chunks of one node, sorted by (kind, rung).
    let kinds: Vec<_> = r.node_entries(n).iter().map(|e| e.key.kind()).collect();
    assert_eq!(kinds, [ChunkKind::Terrain, ChunkKind::Instances, ChunkKind::Collision, ChunkKind::Ground]);
    assert!(r.node_entries(earth_node(12, 0, 0)).is_empty());
    let l10 = earth_node(10, 250, 500);
    assert_eq!(r.node_entries(l10).len(), 1);
    assert_eq!(r.node_entries(l10)[0].key, ChunkKey::layer(l10, "base_hubs"));

    // Children present: parent of (1000..1008, 2000..2008) at L11 has all four children.
    let p = earth_node(11, 500, 1000);
    assert_eq!(r.children_present(p), p.children().unwrap().to_vec());
    let edge: NodeId = earth_node(11, 503, 1003);
    assert_eq!(r.children_present(edge).len(), 4);
    assert!(r.children_present(earth_node(11, 504, 1000)).is_empty());
    assert!(r.children_present(earth_node(10, 250, 500)).is_empty()); // only grandchildren
}

#[test]
fn pack_metadata_and_licence_manifest() {
    let mut w = PackWriter::new(3, 7);
    w.add_licence("gebco").unwrap();
    assert!(w.add_licence("GEBCO").is_err());
    let m = ed_pack::ChunkMeta::new(7, 3).licence("overture-buildings");
    w.insert(ChunkKey::asset(asset("a"), ChunkKind::Material), Chunk::encode(&sample_material(), &m).unwrap()).unwrap();
    w.insert(ChunkKey::asset(asset("b"), ChunkKind::Mesh), Chunk::encode(&sample_mesh(), &meta()).unwrap()).unwrap();
    let (bytes, _) = w.to_bytes().unwrap();
    let r = PackReader::open(bytes).unwrap();
    assert_eq!(r.meta().data_version, 3);
    assert_eq!(r.meta().gen_version, 7);
    assert_eq!(r.meta().licences, ["gebco", "openstreetmap", "overture-buildings"]);
    r.verify_all().unwrap();
}

#[test]
fn flipped_bit_in_a_payload_fails_the_read() {
    let pairs = corpus();
    let (bytes, _) = build(&pairs);
    let r = PackReader::open(bytes.clone()).unwrap();
    let e = *r.entry(&pairs[1].0).unwrap();
    for at in [e.offset + 70, e.offset + e.len as u64 - 1, e.offset] {
        let mut m = bytes.clone();
        m[at as usize] ^= 0x10;
        let r = PackReader::open(m).unwrap(); // manifest untouched, so opening works
        assert!(matches!(r.read(&pairs[1].0), Err(Error::HashMismatch { .. })));
        assert!(r.verify_all().is_err());
        // Other chunks are unaffected.
        assert!(r.read(&pairs[0].0).is_ok());
    }
    // A flipped bit in the manifest fails the open.
    let mut m = bytes.clone();
    let last = m.len() - 5;
    m[last] ^= 1;
    assert!(matches!(PackReader::open(m), Err(Error::HashMismatch { .. })));
}

#[test]
fn truncated_and_wrong_version_packs_are_errors() {
    let pairs: Vec<_> = corpus().into_iter().take(20).collect();
    let (bytes, _) = build(&pairs);
    for n in (0..bytes.len()).step_by(7) {
        assert!(PackReader::open(bytes[..n].to_vec()).is_err(), "opened at {n}");
    }
    let mut m = bytes.clone();
    m[4..8].copy_from_slice(&2u32.to_le_bytes());
    assert!(matches!(PackReader::open(m), Err(Error::UnsupportedPackVersion(2))));
    let mut m = bytes.clone();
    m[0] = b'Z';
    assert!(matches!(PackReader::open(m), Err(Error::BadMagic { .. })));
}

#[test]
fn fuzz_mutated_packs_never_panic() {
    let pairs: Vec<_> = corpus().into_iter().step_by(9).collect();
    let (bytes, _) = build(&pairs);
    let mut rng = Rng(0xC0FFEE);
    let mut opened = 0;
    for round in 0..3000 {
        let mut m = bytes.clone();
        match round % 4 {
            0 => {
                // Mutate the header or manifest (structural fields).
                for _ in 0..1 + rng.below(4) {
                    let at = if rng.below(2) == 0 { rng.below(64) } else { m.len() - 1 - rng.below(m.len().min(2000)) };
                    m[at] = rng.next() as u8;
                }
            }
            1 => {
                let at = rng.below(m.len());
                m[at] ^= 1 << rng.below(8);
            }
            2 => m.truncate(rng.below(m.len())),
            _ => {
                for _ in 0..1 + rng.below(16) {
                    let at = rng.below(m.len());
                    m[at] = rng.next() as u8;
                }
            }
        }
        if let Ok(r) = PackReader::open(m) {
            opened += 1;
            for e in r.entries().to_vec() {
                if let Ok(c) = r.read(&e.key) {
                    let _ = c.decode::<Mesh>();
                    let _ = c.decode::<Instances>();
                    let _ = c.decode::<Opaque>();
                }
            }
            let _ = r.verify_all();
            let _ = r.children_present(earth_node(11, 500, 1000));
        }
    }
    // Payload-only flips leave the manifest intact, so some mutants must have opened.
    assert!(opened > 0);
}

#[test]
fn patch_pack_overrides_in_mount_order() {
    let k = ChunkKey::node(earth_node(12, 5, 5), ChunkKind::Terrain, Rung::R3);
    let only_base = ChunkKey::node(earth_node(12, 6, 6), ChunkKind::Terrain, Rung::R3);
    let mut base = PackWriter::new(1, 1);
    base.insert(k, opaque(ChunkKind::Terrain, vec![1; 10])).unwrap();
    base.insert(only_base, opaque(ChunkKind::Terrain, vec![3; 10])).unwrap();
    let mut patch = PackWriter::new(1, 2);
    patch.insert(k, opaque(ChunkKind::Terrain, vec![2; 10])).unwrap();
    let base = PackReader::open(base.to_bytes().unwrap().0).unwrap();
    let patch = PackReader::open(patch.to_bytes().unwrap().0).unwrap();

    let mut m = Mounts::new();
    m.mount(Box::new(base));
    m.mount(Box::new(patch));
    let got: Opaque = m.get(&k).unwrap().decode::<Opaque>().map(|o| Opaque { data: Cow::Owned(o.data.into_owned()), ..o }).unwrap();
    assert_eq!(got.data.as_ref(), &[2u8; 10]);
    let c = m.get(&only_base).unwrap();
    assert_eq!(c.decode::<Opaque>().unwrap().data.as_ref(), &[3u8; 10]);
    let missing = ChunkKey::node(earth_node(12, 7, 7), ChunkKind::Terrain, Rung::R3);
    assert!(matches!(m.get(&missing), Err(Error::KeyNotFound(_))));
}

#[test]
fn pack_file_source_mapped_and_unmapped() {
    let pairs = corpus();
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let path = dir.join("ed_pack_source_test.ewpack");
    let mut w = PackWriter::new(3, 7);
    for (k, c) in &pairs {
        w.insert(*k, c.clone()).unwrap();
    }
    let s = w.write_file(&path).unwrap();
    assert_eq!(s.file_hash, ChunkHash::of(&std::fs::read(&path).unwrap()));

    let mapped = PackFileSource::open(&path).unwrap();
    let unmapped = PackFileSource::open_unmapped(&path).unwrap();
    for (k, c) in &pairs {
        assert_eq!(mapped.get(k).unwrap().hash(), c.hash());
        assert_eq!(unmapped.get(k).unwrap().hash(), c.hash());
    }
    // Zero-copy: borrowed from the map, payload 16-aligned in the file.
    let (k, _) = pairs.iter().find(|(k, _)| k.kind() == ChunkKind::Mesh).unwrap();
    let c = mapped.reader().read_borrowed(k).unwrap();
    assert!(c.is_borrowed());
    let e = mapped.reader().entry(k).unwrap();
    assert_eq!((e.offset as usize + c.payload_offset()) % 16, 0);
    let mesh: Mesh = c.decode().unwrap();
    assert!(matches!(mesh.vertex_data, Cow::Borrowed(_)));
    let base = mapped.reader().storage().as_slice().unwrap().as_ptr() as usize;
    assert_eq!((mesh.vertex_data.as_ptr() as usize - base) % 16, 0);
    assert_eq!((mesh.index_data.as_ptr() as usize - base) % 16, 0);
    // Shared across worker threads.
    std::thread::scope(|s| {
        for t in 0..4 {
            let (src, pairs) = (&mapped, &pairs);
            s.spawn(move || {
                for (k, c) in pairs.iter().skip(t).step_by(4) {
                    assert_eq!(src.get(k).unwrap().hash(), c.hash());
                }
            });
        }
    });
    std::fs::remove_file(&path).unwrap();
}

/// A fixed pack whose file hash must not change on any platform. If this fails, the on-disk
/// format changed: bump `PACK_VERSION` or the kind's schema, and update this constant.
#[test]
fn golden_pack_hash() {
    let mut w = PackWriter::new(1, 1);
    w.add_licence("natural-earth").unwrap();
    let m = ChunkMeta::new(1, 1).licence("openstreetmap").provenance("golden");
    let n = earth_node(13, 4242, 1717);
    w.insert(ChunkKey::node(n, ChunkKind::Terrain, Rung::R3), Chunk::build(ChunkKind::Terrain, &m, b"terrain").unwrap())
        .unwrap();
    w.insert(ChunkKey::node(n, ChunkKind::Instances, Rung::R0), Chunk::encode(&sample_instances(3, 5), &m).unwrap())
        .unwrap();
    w.insert(ChunkKey::asset(AssetId::from_path("mesh/golden"), ChunkKind::Mesh), Chunk::encode(&sample_mesh(), &m).unwrap())
        .unwrap();
    w.insert(ChunkKey::asset(AssetId::from_path("tex/golden"), ChunkKind::Texture), Chunk::encode(&sample_texture(), &m).unwrap())
        .unwrap();
    w.insert(ChunkKey::asset(AssetId::from_path("mat/golden"), ChunkKind::Material), Chunk::encode(&sample_material(), &m).unwrap())
        .unwrap();
    w.insert(ChunkKey::asset(AssetId::from_path("prefab/golden"), ChunkKind::Prefab), Chunk::encode(&sample_prefab(), &m).unwrap())
        .unwrap();
    w.insert(
        ChunkKey::asset(AssetId::from_path("col/golden"), ChunkKind::CollisionAsset),
        Chunk::encode(&sample_collision(true), &m).unwrap(),
    )
    .unwrap();
    let (bytes, s) = w.to_bytes().unwrap();
    println!("golden pack: {} bytes, hash {}", bytes.len(), s.file_hash);
    assert_eq!(s.file_hash.to_string(), GOLDEN);
}

const GOLDEN: &str = "da3906c991ec465519f4df6a5b606e519ea3c5f9ee6eee80ec2f08f7d72117c2";

/// P4 at the contract's scale: 100,000 keys, binary search on a mapped file, 1,000 distinct
/// bodies stored once.
#[test]
fn hundred_thousand_keys_on_a_mapped_file() {
    let distinct: Vec<Chunk<'static>> =
        (0..1000u32).map(|i| opaque(ChunkKind::Ground, i.to_le_bytes().repeat(8))).collect();
    let mut w = PackWriter::new(1, 1);
    let mut keys = Vec::with_capacity(100_000);
    for i in 0..100_000u32 {
        let k = ChunkKey::node(earth_node(16, i % 400, i / 400), ChunkKind::Ground, Rung::R0);
        w.insert(k, distinct[(i % 1000) as usize].clone()).unwrap();
        keys.push(k);
    }
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ed_pack_100k.ewpack");
    let s = w.write_file(&path).unwrap();
    assert_eq!((s.keys, s.chunks), (100_000, 1000));
    let src = PackFileSource::open(&path).unwrap();
    let t = std::time::Instant::now();
    for (i, k) in keys.iter().enumerate() {
        assert_eq!(src.reader().resolve(k), Some(distinct[i % 1000].hash()));
    }
    let per = t.elapsed().as_secs_f64() * 1e9 / keys.len() as f64;
    println!("100k keys: {} bytes, lookup {per:.0} ns/key", s.bytes);
    assert_eq!(src.get(&keys[54_321]).unwrap().hash(), distinct[321].hash());
    drop(src);
    std::fs::remove_file(&path).unwrap();
}
