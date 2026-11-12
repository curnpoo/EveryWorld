//! Planet runtime: quadtree selection by screen error, horizon culling, node residency, height truth, chunk requests.
//!
//! Allowed workspace dependencies: `ed_geo`, `ed_pack`, `ed_runtime`, `ed_space`. The list is enforced by
//! `cargo xtask check-deps` (rules in `xtask/deps.toml`).
//!
//! Phase 2 W1 (`docs/phases/phase-2.md`, P2-D1..D7): [`Planet::select`] builds the wanted
//! tree under a byte budget and the parent-until-children draw set; [`Planet::height`] is the
//! one height truth. No IO and no GPU here: the loader implements [`Residency`].
//!
//! Determinism: lookups may use hash maps; every output is produced by iterating sorted
//! `NodeId` collections, and transcendental functions come from `libm`.

pub mod body;
pub mod cull;
pub mod page;
pub mod quadtree;
pub mod residency;
pub mod select;

pub use body::{Body, BodyPose};
pub use cull::{Frustum, Horizon};
pub use page::{FacePos, HeightPage, PAGE_CELLS, PAGE_SIZE};
pub use quadtree::{Node, Quadtree};
pub use residency::{ByteLru, ChunkMask, FakeResidency, NodeState, Residency};
pub use select::{DrawNode, Planet, SelectConfig, Selection, View, edge_neighbours};
