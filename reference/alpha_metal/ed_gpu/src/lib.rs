//! `ed_gpu`: the only GPU interface `ed_render` may use (docs/plans/m2.md, rule 1).
//!
//! * [`graph`]: pass graph with declared reads/writes, transient lifetimes, culling and an
//!   alias plan. Backends execute a [`graph::CompiledGraph`]; Metal turns the alias plan into
//!   memory aliasing, wgpu just allocates.
//! * [`caps`]: what the backend can do; features without a portable fallback are gated here.
//! * [`timer`]: per-pass GPU time, smoothed, backend-independent.

pub mod caps;
pub mod clipmap;
pub mod cluster;
pub mod cull;
pub mod graph;
pub mod pool;
pub mod timer;

pub use caps::Caps;
pub use graph::{CompiledGraph, Format, GraphBuilder, GraphError, Lifetime, PassId, QueueKind, ResourceDesc, ResourceId};
pub use timer::PassTimes;
