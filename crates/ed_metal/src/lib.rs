//! Native Metal backend. macOS only.
#![cfg(target_os = "macos")]

pub mod context;
pub mod post;
pub mod timers;
pub mod world;

pub use context::MetalContext;
