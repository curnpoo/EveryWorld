//! Asset baking (`ew-bake`, M3): source files to `.ewpack` chunks, engine-free.
//!
//! Allowed workspace dependencies: `ed_geo`, `ed_data`, `ed_world`, `ed_pack` (only `ed_geo`
//! and `ed_pack` are linked). The list is enforced by `cargo xtask check-deps`.
//!
//! - [`geom`]: mesh data and its pack form; [`lod`]: LOD chains with measured error.
//! - [`texture`]: decode, mips, BC7/BC5/BC4, stable page ids.
//! - [`gltf_import`]: glTF 2.0 (meshes, metal-rough materials, skins, hierarchy -> prefab,
//!   WebP, KHR_texture_transform, required-extension allow-list).
//! - [`material`]: material files (TOML) -> material chunks.
//! - [`megascans`]: albedo/normal/roughness/AO/displacement sets -> material + textures.
//! - [`proving`]: the proving-ground generator (27 parts, 100k instances), bake-time only.
//! - [`bake`]: the source TOML and the pack it produces (provenance on every chunk).
//! - [`licence`]: the registry reader and `verify` (licence gate + hash check).
//! - [`fixtures`]: generated glTF files for tests and the determinism corpus.

pub mod bake;
pub mod fixtures;
pub mod geom;
pub mod gltf_import;
pub mod licence;
pub mod lod;
pub mod material;
pub mod megascans;
pub mod proving;
pub mod texture;

/// Generator version written into every chunk header (bump when bake output changes).
pub const GEN_VERSION: u32 = 3;
/// Tool identity recorded in provenance.
pub const TOOL: &str = concat!("ew-bake ", env!("CARGO_PKG_VERSION"));
