# Handoff: native Metal renderer state (2026-11-05)

Read `docs/CHANGES.md` top entries for the per-change detail; this file is the map.

## Goal
Own native Metal renderer (`ed_metal`) matching/beating the Bevy look at ~60 fps, clear day by default, deterministic and
multiplayer-safe, eventually retiring Bevy rendering. Mac first (M3 Pro, iPhone later), Windows later.
Native is the default; `EW_NATIVE=0` returns to Bevy. `ed_game::native` mirrors Bevy `Mesh3d` entities into
`ed_metal::world::WorldRenderer` under a transparent Bevy window. Bevy systems that key on `Camera::is_active` break under native
(`manage_cameras` deactivates the 3D camera): use "any 3D camera" instead.

## Done (all committed on `main`)
- **Renderer core:** reverse-Z, analytic sky, GGX PBR, HDR + normal + depth, splat terrain (12 layers, hex stochastic/non-repeating tiling,
  triplanar, macro variation; skipped beyond 5 km), hemi-octahedral impostors (draw mode 2), GTAO, SSR, bounce, bloom, exposure, tonemap.
- **MetalFX temporal upscaler** (`upscale.rs`), `EW_NATIVE_SCALE`, `EW_NATIVE_NOAA`. Menu: upscaler and AA are mutually exclusive (`AaMode`).
- **Probe GI** (`ed_world::probes::bake`, `ed_game/probe_gi.rs`, `native::send_probes`, `world.metal::probe_gi`): sun-aware bake
  (re-bake on 4 degree sun change), texture slot 8 (3D RGBA16F). `EW_PROBE_GI=0/EW_PROBE_K/EW_PROBE_DEBUG`.
- **Shadows:** 6 cascades (0-3 to 700 m, 4 to 4 km, 5 to 20 km; 4 and 5 are camera-centred spheres so they cache). Cascade 5 casts buildings
  and terrain; casters under 1.5 texels are culled. Per-cascade hash cache (`shadow_keys`, `shadow_skipped`).
- **Contact shadows:** `post.metal::post_contact`, `EW_CONTACT=0` to disable.
- **Boot flames** (`boot_plume.rs`): triangle layered flames, vibration, wind bend, smoke trail, no HDR gain; old emissive cones in `body.rs` hidden.
- **Default weather is clear** (`clouds.rs::weather_override`; `--weather live` or `EW_WEATHER=live` for the real report).
- **Research docs:** `docs/plans/far-buildings-and-nanite.md` (20 km buildings tiers, Nanite-style options).

## Env switches
`EW_NATIVE=0`, `EW_NATIVE_SCALE`, `EW_NATIVE_NOAA`, `EW_NATIVE_NOSSR/NOBOUNCE/NOAO/NOBLOOM/NOGROUND/NOSHADOW`, `EW_NATIVE_DBG`,
`EW_NATIVE_AOVIS`, `EW_GFX=...`, `EW_HOLD_SPACE=1`, `EW_PROBE_GI`, `EW_PROBE_K`, `EW_PROBE_DEBUG`, `EW_CONTACT`, `EW_WEATHER`.

## How to verify headless (keep short; shell tool times out at 120 s, run long ones in background)
`./target/release/earthdrop --headless --fly|--play --lat .. --lon .. --alt .. --heading .. --pitch .. --hour 15 --width 1280 --height 720 --frames 500 --no-wait --screenshot out.png`
`--play` shows the player (boots; with `EW_HOLD_SPACE=1`); `--fly` has none. Streaming is slow (~30 fps), probes need several hundred frames.
Filter FPS with `grep "native: [0-9]* fps"`.

## Open work, in priority order
1. **NYC window frames render detached** from facades (street level). Likely transform/offset in the window geometry mirrored by `native.rs`. See BUGS B13.
2. **Far buildings to 20 km** per `docs/plans/far-buildings-and-nanite.md` (tiers 3-8 km merged footprints, 8-20 km simplified; `ed_world/src/lod.rs` floor 16 km).
   Verify 20 km shadows visually (low sun, NYC) and shadow-map cost.
3. Walking-height check of grass/ground cover and scatter in native (drawn via `ScatterMaterial`, not verified).
4. FSR 4 (community Metal port, macOS 26+) as an extra `AaMode`; SSR/bounce menu toggles.
5. MetalFX with a moving camera (motion vectors are camera-only; ghosting untested); far-tree horizon shadow not ported.
6. Probe-shaded ground may look dark; tune `EW_PROBE_K`/shade floor if the user says so. More grade contrast; Matterhorn raking shadows.
7. Contact-shadow strength (0.45) is a guess; boots untested under MetalFX and with Shift boost.

## Conventions
Commit trailer `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`; push to `origin/main`. Keep `CLAUDE.md`, `AGENTS.md`, `docs/CHANGES.md`
current. The user wants step-by-step progress without check-ins and is near their usage limit.
