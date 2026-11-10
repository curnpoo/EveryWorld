//! Native Metal backend. macOS only.
#![cfg(target_os = "macos")]

pub mod clouds;
pub mod context;
pub mod ocean;
pub mod post;
pub mod timers;
pub mod upscale;
pub mod world;

pub use context::MetalContext;
