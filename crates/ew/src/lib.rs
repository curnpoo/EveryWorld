//! The application: window, input, modes (play, editor), wiring of all engine crates.
//!
//! Allowed workspace dependencies: `ed_geo`, `ed_data`, `ed_world`, `ed_pack`, `ed_planet`, `ed_gpu`, `ed_render`, `ed_sim`, `ed_anim`, `ed_editor`, `ed_store`, `ed_net`, `ed_runtime`, `ed_flight`, `ed_space`, `ed_audio`, `ed_engine_audio`, `ed_assets`. The list is enforced by
//! `cargo xtask check-deps` (rules in `xtask/deps.toml`).
//!
//! Status: Phase 1, M3 (content from packs, [`content`]). Earlier: M1b. The `ew` binary (`src/main.rs`) opens a window with a Metal surface
//! ([`windowed`]) or renders offscreen with no window ([`headless`], P1-D15). Both run the
//! main loop input -> fixed 60 Hz step ([`fixed_step`]) -> frame build -> submit, and render
//! with the step fraction for interpolation (P1-D16, P1-D17). The simulation plugs in through
//! [`fixed_step::Simulation`]; for now the only simulations are cameras.

pub mod args;
pub mod authored;
pub mod camera;
pub mod collide;
pub mod content;
pub mod fixed_step;
pub mod game;
pub mod harness;
pub mod headless;
pub mod hot;
pub mod perf;
pub mod play;
pub mod proving;
pub mod real_materials;
pub mod route;
pub mod session;
pub mod terrain;
pub mod terrain_collide;
pub mod walker;
pub mod windowed;

/// One line describing the device capabilities (summary files and logs).
pub fn caps_line(c: &ed_gpu::Caps) -> String {
    format!(
        "caps: backend={:?} device=\"{}\" unified_memory={} memory_budget={} MiB bindless_tier={} tables(tex/buf/smp)={}/{}/{} \
         timers={:?} indirect_count={} native_mdi={} mesh_shaders={} ray_tracing={} atomics_64={} hdr_output={}",
        c.backend,
        c.device_name,
        c.unified_memory,
        c.memory_budget >> 20,
        c.bindless_tier,
        c.max_bindless_textures,
        c.max_bindless_buffers,
        c.max_bindless_samplers,
        c.timers,
        c.indirect_count,
        c.native_multi_draw_indirect,
        c.mesh_shaders,
        c.ray_tracing,
        c.atomics_64,
        c.hdr_output
    )
}
