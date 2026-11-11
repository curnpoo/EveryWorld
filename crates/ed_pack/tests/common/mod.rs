#![allow(dead_code)]
//! Shared fixtures: a fixed-seed PRNG and one sample value per chunk kind.

use std::borrow::Cow;
use std::collections::BTreeMap;

use ed_geo::{BodyId, NodeId, ObjectId};
use ed_pack::kinds::*;
use ed_pack::{AssetId, Chunk, ChunkHash, ChunkKind, ChunkMeta};

/// xorshift64*: deterministic, no dependencies.
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    pub fn fill(&mut self, buf: &mut [u8]) {
        for c in buf.chunks_mut(8) {
            let v = self.next().to_le_bytes();
            c.copy_from_slice(&v[..c.len()]);
        }
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            v.swap(i, self.below(i + 1));
        }
    }
}

pub fn earth_node(level: u8, x: u32, y: u32) -> NodeId {
    NodeId::new(BodyId(3), 2, level, x, y).unwrap()
}

pub fn meta() -> ChunkMeta {
    ChunkMeta::new(7, 3).licence("openstreetmap").provenance("test:fixture")
}

pub fn asset(name: &str) -> AssetId {
    AssetId::from_path(name)
}

pub fn sample_mesh() -> Mesh<'static> {
    // A quad (2 triangles) as LOD 0 and one triangle as LOD 1. Position f32x3 + uv f32x2.
    let verts: [[f32; 5]; 4] = [[0., 0., 0., 0., 0.], [1., 0., 0., 1., 0.], [1., 1., 0., 1., 1.], [0., 1., 0., 0., 1.]];
    let vertex_data: Vec<u8> = verts.iter().flatten().flat_map(|f| f.to_le_bytes()).collect();
    let idx: [u16; 9] = [0, 1, 2, 0, 2, 3, 0, 1, 2];
    let index_data: Vec<u8> = idx.iter().flat_map(|i| i.to_le_bytes()).collect();
    Mesh {
        attributes: vec![
            VertexAttribute { semantic: VertexSemantic::Position, format: VertexFormat::F32x3, offset: 0 },
            VertexAttribute { semantic: VertexSemantic::Uv0, format: VertexFormat::F32x2, offset: 12 },
        ],
        vertex_stride: 20,
        vertex_count: 4,
        vertex_data: Cow::Owned(vertex_data),
        index_format: IndexFormat::U16,
        index_data: Cow::Owned(index_data),
        lods: vec![
            MeshLod {
                first_index: 0,
                index_count: 6,
                error_m: 0.0,
                sections: vec![MeshSection { first_index: 0, index_count: 6, material_slot: 0 }],
            },
            MeshLod {
                first_index: 6,
                index_count: 3,
                error_m: 0.25,
                sections: vec![MeshSection { first_index: 6, index_count: 3, material_slot: 0 }],
            },
        ],
        bounds: Bounds { centre: [0.5, 0.5, 0.0], radius: 0.71, min: [0.0; 3], max: [1.0, 1.0, 0.0] },
        material_slots: vec!["facade".into()],
    }
}

pub fn sample_texture() -> Texture<'static> {
    // 8x8 BC7 with 3 mips (4 blocks, 1, 1).
    let mips = [64usize, 16, 16]
        .iter()
        .enumerate()
        .map(|(i, n)| Cow::Owned((0..*n).map(|b| (b * 7 + i) as u8).collect()))
        .collect();
    Texture { format: TextureFormat::Bc7, role: ColourRole::Srgb, width: 8, height: 8, layers: 1, page_id: 0xABCD, mips }
}

pub fn sample_material() -> Material {
    let mut m = Material::new(MaterialModel::Pbr);
    m.params.insert("base_colour".into(), Param::Vec4([0.8, 0.7, 0.6, 1.0]));
    m.params.insert("roughness".into(), Param::Float(0.6));
    m.textures.insert("albedo".into(), asset("tex/brick_albedo.png"));
    m.textures.insert("normal".into(), asset("tex/brick_normal.png"));
    m
}

pub fn sample_collision(for_asset: bool) -> Collision {
    Collision {
        for_asset,
        shapes: vec![
            CollisionShape {
                shape: Shape::Box { centre: [0.0, 1.0, 0.0], half_extents: [1.0, 1.0, 2.0], rotation: [0.0, 0.0, 0.0, 1.0] },
                surface: 1,
            },
            CollisionShape { shape: Shape::Sphere { centre: [3.0, 0.0, 0.0], radius: 0.5 }, surface: 2 },
            CollisionShape { shape: Shape::Capsule { a: [0.0; 3], b: [0.0, 2.0, 0.0], radius: 0.3 }, surface: 3 },
            CollisionShape {
                shape: Shape::ConvexHull { points: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] },
                surface: 4,
            },
            CollisionShape {
                shape: Shape::TriMesh {
                    vertices: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
                    triangles: vec![[0, 1, 2]],
                },
                surface: 5,
            },
            CollisionShape {
                shape: Shape::HeightField {
                    origin: [0.0; 3],
                    cell_size: [2.0, 2.0],
                    columns: 2,
                    rows: 3,
                    heights: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                },
                surface: 6,
            },
        ],
    }
}

pub fn sample_prefab() -> Prefab {
    let t = Transform { translation: [1.0, 0.0, 2.0], ..Default::default() };
    Prefab {
        nodes: vec![
            PrefabNode { parent: None, name: "root".into(), transform: Transform::default(), mesh: None, material: None },
            PrefabNode {
                parent: Some(0),
                name: "hall".into(),
                transform: t,
                mesh: Some(asset("mesh/hall.glb#0")),
                material: Some(asset("mat/stone")),
            },
            PrefabNode { parent: Some(1), name: "roof".into(), transform: t, mesh: Some(asset("mesh/hall.glb#1")), material: None },
        ],
        footprint: vec![[-10.0, -5.0], [10.0, -5.0], [10.0, 5.0], [-10.0, 5.0]],
        points: vec![PrefabPoint { name: "door_main".into(), kind: PointKind::Door, transform: t }],
        rung_meshes: vec![(ed_pack::Rung::R1, asset("mesh/hall_shell")), (ed_pack::Rung::R2, asset("mesh/hall_proxy"))],
    }
}

pub fn sample_skeleton() -> Skeleton {
    let mut ib = [0.0f32; 16];
    for i in 0..4 {
        ib[i * 5] = 1.0;
    }
    Skeleton {
        mesh: asset("mesh/cat"),
        joints: vec![
            Joint { parent: None, name: "root".into(), rest: Transform::default(), inverse_bind: ib },
            Joint { parent: Some(0), name: "thigh.L".into(), rest: Transform { translation: [0.1, 0.5, 0.0], ..Default::default() }, inverse_bind: ib },
        ],
    }
}

pub fn sample_instances(n: usize, seed: u64) -> Instances {
    let mut rng = Rng(seed);
    let v = (0..n)
        .map(|i| Instance {
            object: ObjectId::generated("overture-buildings", &format!("f{i}"), "building.part", 0),
            transform: Transform {
                translation: [(rng.below(1000) as f32) * 0.5, 0.0, i as f32],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0 + (i % 3) as f32, 1.0],
            },
            asset: asset(&format!("mesh/part{}", i % 5)),
            material: (i % 2 == 0).then(|| asset("mat/brick")),
            variant: (i % 4) as u16,
            flags: i as u32,
        })
        .collect();
    Instances::sorted(v)
}

/// One chunk of every kind (typed where typed, opaque for reserved kinds).
pub fn every_kind() -> Vec<Chunk<'static>> {
    let m = meta();
    let mesh = Chunk::encode(&sample_mesh(), &m).unwrap();
    let mut out = vec![
        Chunk::encode(&Clusters { mesh: mesh.hash(), data: Cow::Owned(vec![1, 2, 3, 4, 5]) }, &m).unwrap(),
        mesh,
        Chunk::encode(&sample_texture(), &m).unwrap(),
        Chunk::encode(&sample_material(), &m).unwrap(),
        Chunk::encode(&sample_collision(false), &m).unwrap(),
        Chunk::encode(&sample_collision(true), &m).unwrap(),
        Chunk::encode(&sample_prefab(), &m).unwrap(),
        Chunk::encode(&sample_skeleton(), &m).unwrap(),
        Chunk::encode(&sample_instances(10, 1), &m).unwrap(),
        Chunk::encode(&LayerText { text: Cow::Borrowed("") }, &m).unwrap(),
    ];
    for k in ChunkKind::ALL.iter().filter(|k| k.is_reserved()) {
        out.push(Chunk::encode(&Opaque { kind: *k, data: Cow::Owned(vec![k.id() as u8; 33]) }, &m).unwrap());
    }
    out
}

pub fn opaque(kind: ChunkKind, bytes: Vec<u8>) -> Chunk<'static> {
    Chunk::encode(&Opaque { kind, data: Cow::Owned(bytes) }, &meta()).unwrap()
}

pub fn hash_hex(h: &ChunkHash) -> String {
    h.to_string()
}

pub fn btree<K: Ord, V>(v: Vec<(K, V)>) -> BTreeMap<K, V> {
    v.into_iter().collect()
}
