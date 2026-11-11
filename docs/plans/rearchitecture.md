# The alpha: audit and migration map

What the Bevy alpha (`alpha-early`) was, why it is being replaced, and what happens to each
piece of it. The target design is in `../ARCHITECTURE.md`; this file only looks backward and
maps old to new. First written 2026-11-08; revised 2026-11-10 after the foundation review.

## 1. What the alpha proved

Built in four days: real-Earth streaming with elevation and roads, procedural buildings and
trees in 17 regional styles, a robot courier with rocket boots, prototype cars and planes,
volumetric clouds, an atmosphere, a native Metal renderer with ambient occlusion,
reflections and MetalFX upscaling, and ghost multiplayer. The idea works and the look is
close to what the owner wants.

## 2. What the owner reported (2026-11-10)

The Metal path "runs not great, but looks pretty good, loads slowly, has gaps". Each symptom
has a structural cause:

| Symptom | Cause in the alpha | Replaced by |
|---|---|---|
| Runs poorly | Merged per-tile meshes: about 16.5 million triangles in a city view, drawn again for the depth prepass and every shadow cascade; no per-object culling or LOD | Part instances, GPU culling, LOD per instance, far rungs (F2, F3) |
| Loads slowly | Every tile generated from raw data at full detail on the player's machine, then uploaded whole | Baked chunks, coarse first, staged uploads (F4) |
| Gaps | Render tiles, data tiles and caches addressed differently; vector-tile roads clipped at tile edges; tiles shown before complete; a separate globe above about 200 km | One address, edge ownership, parent-until-children, one planet (F1, D2, F8) |
| Looks good | Atmosphere, clouds, tone mapping, terrain material, upscaling | Kept as the look reference and re-implemented (P1-D18) |

## 3. Audit findings

### 3.1 Sound, and kept

- f64 body-fixed truth with camera-relative f32 rendering and reverse-Z.
- The engine-free crates: `ed_geo`, `ed_data`, `ed_world`, `ed_flight`, `ed_space`,
  `ed_runtime`, `ed_engine_audio`, `ed_audio`, and `ed_net` as a protocol reference. They
  are unit-tested and do not depend on Bevy. They are inefficient in places but express the
  right rules; they are covered by golden hashes first (Phase 0) and optimised or reshaped
  after (Phase 3).
- Design knowledge: single height truth, the roads v2 height solver, region and
  architecture tables, the biome model, the measurement tooling, the BeamNG.Earth lessons.
- The determinism and data-pinning rules.

### 3.2 Shortcomings

| # | Shortcoming | Blocks |
|---|---|---|
| S1 | Three renderers: Bevy, a native Metal renderer fed by mirroring Bevy entities, and a menu globe | Every visual change costs two or three times; GPU-driven drawing is impossible through the mirror |
| S2 | Per-tile merged geometry and impostor cards | Density, frame rate, destruction, interiors |
| S3 | Two Earth representations with a switch near 200 km | The core ground-to-orbit experience |
| S4 | Bevy and Avian throughout the game crate; physics not repeatable across machines | Custom engine, server authority |
| S5 | The generator emits merged vertex buffers, not part instances | The modular kit, instancing, editing, far rungs |
| S6 | Data: vector tiles drop road detail; 10 m imagery used as near ground; two non-shippable datasets | Driveable roads, ground quality, selling the game |
| S7 | Server is a stub | The shared universe |
| S8 | No skinned characters or animation | Characters and NPCs |
| S9 | No asset pipeline | Scanned assets, prefabs |
| S10 | Effort spent tuning a renderer that is being retired | Everything else |
| S11 | No far-field representation: a distant city needs all its geometry generated | Seeing cities from far away, orbit views (added 2026-11-10) |
| S12 | No stable identity for generated objects | An editor, persistence of edits, multiplayer deltas (added 2026-11-10) |
| S13 | No streaming backend: the client is the only place content is made | Load time (added 2026-11-10) |

### 3.3 Stopped

- Tuning per-tile triangle counts and the Bevy-to-Metal mirror.
- New Bevy features and visual bug fixes on the alpha (crash fixes only, on `alpha-early`).
- Live ground physics seen from orbit (parked records instead).
- Adopting an external renderer as the engine (F12).

## 4. Migration map

| Alpha piece | Fate |
|---|---|
| `ed_geo` | Keep. Add cube-sphere and `NodeId` maths. `TileId` moves behind `ed_data` |
| `ed_data` | Keep. Add Overture transportation and OSM tag readers; registry-driven sources |
| `ed_world` | Keep the rules. Replace merged-mesh output with per-rung output (instances, shells, proxies, layers), in Phase 3, under golden hashes |
| `ed_world::roads` v2 | Keep the solver; new input from Overture; node-space output (`streets.md`) |
| `ed_world::lod`, far-building tiers | Superseded by the ladder; the alpha notes in `../alpha/plans/far-buildings-and-nanite.md` are background |
| `ed_flight`, `ed_space`, audio crates | Keep |
| `ed_runtime` | Keep; becomes the job and residency layer for streaming |
| `ed_metal` and alpha `ed_gpu` (cluster, pool, graph, cull experiments) | Move to `reference/alpha_metal/` outside the workspace. Look reference and shader-maths source. The new `ed_gpu` is written against `contracts/rhi.md` |
| Atmosphere, clouds, post, MetalFX code | Re-implemented on the new renderer; compared to Phase 0 captures |
| Ocean FFT, probe GI, shadow caching | Not ported (D13, D14) |
| `ed_game` (Bevy app), vendored `bevy_pbr` | Removed from `main` after the Phase 0 baseline; remains on `alpha-early` |
| Avian | Replaced by Box3D behind `ed_sim` |
| Procedural robot body | Replaced by skinned characters; IK lessons go to `ed_anim` |
| Globe, ocean globe, menu globe | Replaced by `ed_planet` |
| `ed_net` ghost protocol | Reference for the new protocol |
| `ed_server` | Rebuilt |
| `alpha/BUGS.md` | Replacement criteria; closed as each subsystem is replaced |

## 5. Acceptance (what "replaced" means)

- **Seamless:** one continuous recording street → orbit → Moon → street, with no holes,
  switches or texture-regime changes.
- **Performance:** High tier 60 fps on the reference scenes and the golden-path route on the
  M3 Pro, with fixed memory while streaming.
- **Loading:** spawn-to-complete-view and time-to-full-detail within the budgets set from
  the Phase 0 baseline.
- **Look:** clearly better than the Phase 0 look captures, which are a floor (P1-D25).
- **Authoring:** a building, a marker and a mission placed in the editor survive a re-bake
  and a data-version update.
- **Persistence and multiplayer:** as in `../ROADMAP.md` Phases 6 and 7.

## 6. References

- Alpha renderer state: `../alpha/HANDOFF.md`. Alpha engine notes: `../alpha/plans/engine.md`.
- Roads v2: `../alpha/plans/roads.md`. BeamNG.Earth lessons:
  `../alpha/plans/beamng-earth-lessons.md`.
- Foundation review of 2026-11-10: recorded in `../CHANGES.md` and as the F decisions in
  `../DECISIONS.md`.
