//! The GPU interface (RHI): device, resources with states, bindless tables, timers, capability probe. Backends are private modules. Contract: docs/contracts/rhi.md.
//!
//! Allowed workspace dependencies: none inside the workspace. The list is enforced by
//! `cargo xtask check-deps` (rules in `xtask/deps.toml`).
//!
//! Empty until Phase 1; see `docs/ARCHITECTURE.md` section 3.
