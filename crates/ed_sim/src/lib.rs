//! Fixed-step simulation: physics, character mover, vehicles, water query. No render types.
//!
//! Contract: `docs/contracts/sim.md`. Allowed workspace dependencies: `ed_geo`, `ed_flight`,
//! `ed_space`; enforced by `cargo xtask check-deps` (rules in `xtask/deps.toml`).
//!
//! The rigid-body backend is vendored Box3D (`vendor/box3d`, built by `build.rs` in double
//! precision with one worker and no FMA contraction) behind a small C shim
//! (`csrc/ed_box3d_shim.c`). Nothing of Box3D appears in the public API.
//!
//! Frame: the region frame, x east, y north, z up, metres, f64 positions. One [`World::step`]
//! advances exactly [`DT`] with [`SUBSTEPS`] solver substeps; the simulation never reads a clock.

mod accum;
mod codec;
mod debug_draw;
pub mod demo;
mod ffi;
mod frame;
mod mover;
mod replay;
mod types;
mod world;

pub use accum::Accumulator;
pub use debug_draw::{DebugDraw, Rgba};
pub use replay::{Divergence, Recorder, Replay};
pub use types::{
    BodyDesc, BodyKind, Command, CommandKind, Entity, Filter, GeomId, Geometry, HeightField, Material, MoverDesc,
    MoverInput, MoverState, QueryFilter, Shape, ShapeDesc, TriMesh, layers,
};
pub use world::{BodyState, CastShape, RayHit, ShapeHit, SimConfig, SimInput, Snapshot, StepOutput, World};

/// Simulation ticks per second.
pub const TICK_HZ: u32 = 60;
/// Solver substeps per tick.
pub const SUBSTEPS: u32 = 4;
/// Seconds per tick.
pub const DT: f64 = 1.0 / TICK_HZ as f64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SimError {
    /// Bytes are not a valid snapshot or replay.
    Corrupt(&'static str),
    /// A snapshot or replay from a different build (backend layout or version).
    Incompatible(&'static str),
    /// A description the backend cannot accept (for example a mesh on a dynamic body).
    InvalidDesc(&'static str),
    /// The backend refused (out of worlds, hull construction failed, ...).
    Backend(&'static str),
}

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimError::Corrupt(s) => write!(f, "corrupt data: {s}"),
            SimError::Incompatible(s) => write!(f, "incompatible data: {s}"),
            SimError::InvalidDesc(s) => write!(f, "invalid description: {s}"),
            SimError::Backend(s) => write!(f, "physics backend: {s}"),
        }
    }
}

impl std::error::Error for SimError {}
