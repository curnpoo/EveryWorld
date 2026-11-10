//! The renderer: instance pools, GPU culling, geometry providers, lighting, atmosphere, post, picking. Talks to the GPU only through `ed_gpu`.
//!
//! Allowed workspace dependencies: `ed_geo`, `ed_gpu`, `ed_pack`, `ed_planet`. The list is enforced by
//! `cargo xtask check-deps` (rules in `xtask/deps.toml`).
//!
//! Empty until Phase 1; see `docs/ARCHITECTURE.md` section 3.
