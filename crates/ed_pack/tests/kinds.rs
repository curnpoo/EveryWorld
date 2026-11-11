//! P1/P3: every chunk kind round-trips; wrong kind, schema, codec and corrupt bytes are typed
//! errors.

mod common;

use std::borrow::Cow;

use common::*;
use ed_pack::kinds::*;
use ed_pack::{CHUNK_HEADER_LEN, Chunk, ChunkBody, ChunkKind, Error};

fn round_trip<'a, B>(body: &B) -> Chunk<'static>
where
    B: for<'b> ChunkBody<'b> + PartialEq + std::fmt::Debug,
{
    let c = Chunk::encode(body, &meta()).unwrap();
    let parsed = Chunk::parse(c.bytes().to_vec()).unwrap();
    assert_eq!(parsed.hash(), c.hash());
    assert_eq!(parsed.meta(), c.meta());
    assert_eq!(&parsed.decode::<B>().unwrap(), body);
    assert_eq!(c.payload_offset() % 16, 0);
    c
}

#[test]
fn mesh_round_trips_with_lod_chain() {
    let m = sample_mesh();
    let c = Chunk::encode(&m, &meta()).unwrap();
    let back = Chunk::parse(c.bytes().to_vec()).unwrap();
    let d: Mesh = back.decode().unwrap();
    assert_eq!(d, m);
    assert_eq!(d.lods.len(), 2);
    assert_eq!(d.lods[1].error_m, 0.25);
    // Vertex and index streams are 16-aligned relative to the payload.
    let p = back.payload();
    let vo = d.vertex_data.as_ptr() as usize - p.as_ptr() as usize;
    let io = d.index_data.as_ptr() as usize - p.as_ptr() as usize;
    assert_eq!((vo % 16, io % 16), (0, 0));
    // Decoding borrows (no copy of the streams).
    assert!(matches!(d.vertex_data, Cow::Borrowed(_)));
}

#[test]
fn texture_material_collision_prefab_instances_clusters_layer_round_trip() {
    let t = Chunk::encode(&sample_texture(), &meta()).unwrap();
    let back = Chunk::parse(t.bytes().to_vec()).unwrap();
    let d: Texture = back.decode().unwrap();
    assert_eq!(d, sample_texture());
    for m in &d.mips {
        assert_eq!((m.as_ptr() as usize - back.payload().as_ptr() as usize) % 16, 0);
    }
    round_trip(&sample_material());
    let c = round_trip(&sample_collision(false));
    assert_eq!(c.kind(), ChunkKind::Collision);
    let c = round_trip(&sample_collision(true));
    assert_eq!(c.kind(), ChunkKind::CollisionAsset);
    round_trip(&sample_prefab());
    round_trip(&sample_instances(50, 9));

    let mesh = Chunk::encode(&sample_mesh(), &meta()).unwrap();
    let cl = Clusters { mesh: mesh.hash(), data: Cow::Owned(vec![9u8; 100]) };
    let c = Chunk::encode(&cl, &meta()).unwrap();
    assert_eq!(c.decode::<Clusters>().unwrap(), cl);

    let lt = LayerText { text: Cow::Borrowed("[[op]]\n") };
    let c = Chunk::encode(&lt, &meta()).unwrap();
    assert_eq!(c.decode::<LayerText>().unwrap(), lt);
}

#[test]
fn reserved_kinds_are_opaque_schema_0() {
    for k in ChunkKind::ALL.iter().filter(|k| k.is_reserved()) {
        let c = opaque(*k, vec![1, 2, 3]);
        assert_eq!(c.schema(), 0);
        let p = Chunk::parse(c.bytes().to_vec()).unwrap();
        let o: Opaque = p.decode().unwrap();
        assert_eq!((o.kind, o.data.as_ref()), (*k, &[1u8, 2, 3][..]));
    }
    // A typed kind cannot be written as opaque.
    let e = Chunk::encode(&Opaque { kind: ChunkKind::Mesh, data: Cow::Borrowed(&[]) }, &meta()).unwrap_err();
    assert!(matches!(e, Error::Invalid(_)));
}

#[test]
fn every_kind_has_a_chunk_and_parses() {
    let all = every_kind();
    let mut kinds: Vec<_> = all.iter().map(|c| c.kind()).collect();
    kinds.sort();
    kinds.dedup();
    assert_eq!(kinds.len(), ChunkKind::ALL.len());
    for c in all {
        let p = Chunk::parse(c.bytes().to_vec()).unwrap();
        assert_eq!(p.kind(), c.kind());
        assert_eq!(p.licences(), ["openstreetmap"]);
        assert_eq!(p.provenance(), ["test:fixture"]);
        assert_eq!(p.header().gen_version, 7);
        assert_eq!(p.header().data_version, 3);
    }
}

#[test]
fn wrong_kind_is_a_typed_error() {
    let c = Chunk::encode(&sample_mesh(), &meta()).unwrap();
    match c.decode::<Texture>() {
        Err(Error::WrongKind { expected: "Texture", found: ChunkKind::Mesh }) => {}
        other => panic!("{other:?}"),
    }
    assert!(matches!(c.decode::<Opaque>(), Err(Error::WrongKind { .. })));
}

#[test]
fn unknown_kind_schema_and_codec_are_typed_errors() {
    let c = Chunk::encode(&sample_material(), &meta()).unwrap();
    let mut b = c.bytes().to_vec();
    b[6..8].copy_from_slice(&2u16.to_le_bytes());
    assert!(matches!(
        Chunk::parse(b.clone()),
        Err(Error::UnsupportedSchema { kind: ChunkKind::Material, found: 2, supported: 1 })
    ));
    let mut b = c.bytes().to_vec();
    b[4..6].copy_from_slice(&99u16.to_le_bytes());
    assert!(matches!(Chunk::parse(b), Err(Error::UnknownKind(99))));
    let mut b = c.bytes().to_vec();
    b[16] = 1;
    assert!(matches!(Chunk::parse(b), Err(Error::UnsupportedCodec(1))));
    let mut b = c.bytes().to_vec();
    b[0] = b'X';
    assert!(matches!(Chunk::parse(b), Err(Error::BadMagic { .. })));
}

#[test]
fn truncated_chunks_are_errors_at_every_length() {
    for c in every_kind() {
        let b = c.bytes();
        for n in 0..b.len() {
            assert!(Chunk::parse(&b[..n]).is_err(), "{:?} truncated to {n} parsed", c.kind());
        }
        let mut longer = b.to_vec();
        longer.push(0);
        assert!(Chunk::parse(longer).is_err());
    }
}

#[test]
fn a_flipped_bit_anywhere_fails_the_hash_check() {
    let c = Chunk::encode(&sample_mesh(), &meta()).unwrap();
    let h = c.hash();
    let b = c.bytes();
    for i in 0..b.len() {
        for bit in [0, 3, 7] {
            let mut m = b.to_vec();
            m[i] ^= 1 << bit;
            assert!(matches!(Chunk::parse_verified(m, &h), Err(Error::HashMismatch { .. })));
        }
    }
    assert!(Chunk::parse_verified(b, &h).is_ok());
}

#[test]
fn corrupt_payloads_never_panic() {
    // Mutate bytes after the hash would have caught them: the structural decoders must still
    // return errors (or a valid value), never panic.
    let mut rng = Rng(0x5eed_1234);
    for c in every_kind() {
        let b = c.bytes().to_vec();
        for _ in 0..1500 {
            let mut m = b.clone();
            let flips = 1 + rng.below(6);
            for _ in 0..flips {
                let at = CHUNK_HEADER_LEN.min(m.len() - 1) + rng.below(m.len() - CHUNK_HEADER_LEN.min(m.len() - 1));
                m[at] = rng.next() as u8;
            }
            if rng.below(4) == 0 {
                m.truncate(rng.below(m.len() + 1));
            }
            if let Ok(p) = Chunk::parse(m) {
                let _ = p.decode::<Mesh>();
                let _ = p.decode::<Texture>();
                let _ = p.decode::<Material>();
                let _ = p.decode::<Collision>();
                let _ = p.decode::<Prefab>();
                let _ = p.decode::<Instances>();
                let _ = p.decode::<Clusters>();
                let _ = p.decode::<LayerText>();
                let _ = p.decode::<Opaque>();
            }
        }
    }
}

#[test]
fn invalid_bodies_are_rejected_on_encode() {
    let mut m = sample_mesh();
    m.lods[1].error_m = -1.0; // decreasing error
    assert!(Chunk::encode(&m, &meta()).is_err());
    let mut m = sample_mesh();
    m.lods.clear(); // LOD chain is required
    assert!(Chunk::encode(&m, &meta()).is_err());
    let mut t = sample_texture();
    t.mips[1] = Cow::Owned(vec![0; 3]);
    assert!(Chunk::encode(&t, &meta()).is_err());
    let mut i = sample_instances(3, 1);
    i.instances.swap(0, 1); // must be sorted by ObjectId
    assert!(Chunk::encode(&i, &meta()).is_err());
    assert!(Chunk::encode(&sample_material(), &ed_pack::ChunkMeta::new(0, 0).licence("Not A Registry Id")).is_err());
}

#[test]
fn meta_is_canonical_regardless_of_order() {
    let a = ed_pack::ChunkMeta::new(1, 2).licence("osm").licence("gebco").provenance("b").provenance("a");
    let b = ed_pack::ChunkMeta::new(1, 2).licence("gebco").licence("osm").licence("osm").provenance("a").provenance("b");
    let ca = Chunk::encode(&sample_material(), &a).unwrap();
    let cb = Chunk::encode(&sample_material(), &b).unwrap();
    assert_eq!(ca.hash(), cb.hash());
    assert_eq!(ca.licences(), ["gebco", "osm"]);
}
