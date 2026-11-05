//! Engine-free runtime services for EveryWorld (`docs/plans/engine.md`, M1).
//!
//! - [`jobs`]: thread pools with priorities, re-prioritisation and
//!   cancellation. Separate pools for network IO and CPU work, so a slow
//!   download never holds a core that could be building the tile under you.
//! - [`budget`]: per-frame work budgets for main-thread apply steps.
//! - [`residency`]: least-recently-seen bookkeeping with a cap.
//! - [`view`]: conservative sphere-vs-view-cone test.
//! - [`scheduler`]: which tiles to build, in what order, and which to retire.
//! - [`shadows`]: sun cascade plan (splits, reach, fade) from height above ground.
//!
//! Nothing here knows about Bevy. `ed_game::streaming` is the thin glue.

pub mod budget;
pub mod jobs;
pub mod residency;
pub mod scheduler;
pub mod shadows;
pub mod view;
