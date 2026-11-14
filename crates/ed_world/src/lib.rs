//! Procedural world generation from real Earth data.
//!
//! Input: elevation grids + OpenMapTiles vector tiles + climate hints.
//! Output: engine-agnostic mesh data ([`mesh::MeshData`]) and instance lists
//! that the game turns into GPU meshes and physics colliders.
//!
//! The rule of thumb: **real data decides *what* is there, procedural rules
//! decide *how it looks*.** OSM says "a 3-storey building with this footprint
//! is here"; the regional style says "in Provence that means ochre stucco,
//! green shutters and a terracotta hip roof".

pub mod amplify;
pub mod archkit;
pub mod biome;
pub mod bparts;
pub mod buildings;
pub mod cliffs;
pub mod clouds;
pub mod detail_normal;
pub mod farshadow;
pub mod foliage;
pub mod garage;
pub mod globe;
pub mod grass;
pub mod ground_cover;
pub mod ground_scatter;
pub mod ground_tex;
pub mod horizon;
pub mod library;
pub mod lod;
pub mod maprender;
pub mod mesh;
pub mod meshkit;
pub mod missions;
pub mod noise;
pub mod probes;
pub mod project;
pub mod props;
pub mod roads;
pub mod season;
pub mod stand;
pub mod starfield;
pub mod soundmap;
pub mod style;
pub mod surface;
pub mod terrain;
pub mod tilegen;
pub mod trees;
pub mod water;
pub mod wind;

/// Deterministic hash -> [0,1). Same input, same world, every run, every Mac.
pub fn hash01(a: u64, b: u64) -> f32 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_add(0xD1B5_4A32_D192_ED03);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 40) as f32 / (1u64 << 24) as f32
}
