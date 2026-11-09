# Goals — status board

The goals themselves now live in **`docs/VISION.md`** (owner's words, verbatim, with the
systems that deliver each). This file is only the status tracker; it exists so older links
keep working. Update statuses as phases land (`docs/ROADMAP.md`).

| Vision section | Goal | Status (2026-11-08) |
|---|---|---|
| §1 | Guided freedom, merit-based progression, home bases | Not built; `ed_store` + missions service in Phases 5–6 |
| §2 | North-star session loop, solo then multiplayer | Phases 5 and 6 respectively |
| §3 | One continuous planet, no flat-plane/globe switch | Phase 2 spike; alpha had the hard switch at ~200 km |
| §4 | Persistent shared universe, visible distant launches, NPC life | Phase 6 (shards, sky events), Phase 7 (NPC schedules) |
| §5 | Photoreal world + cel characters + solarpunk built world | Phase 3 (renderer + Megascans + toon materials) |
| §6 | Owner GLB → VRM custom avatars; realistic + robot NPCs | `ed_anim` Phase 3–4; avatar platform Phase 7 |
| §7 | Real-feeling vehicles, destruction, knockable trees | Phase 4 (Box3D + car signoff), Phase 7 (destruction) |
| §8 | Handcrafted + guided procedural, authored hubs | Ongoing; kit + placements in Phases 1–4 |
| §9 | Stations, Moon/Mars civilizations, lanes, fourth world | Phase 7+; fourth world is a later design exploration |
| §12 | Building interiors — procedural floor plans + authored interiors, no door-teleports | Kit contract from Phase 3; full interiors Phase 7 |
| §10 | Twin-Earth data library, licensed and pinned | Phase 0 registry; WorldClim + EOX replacements are P0 |
| §11 | Custom engine, Box3D, Ubuntu server, all-in reboot | Decided; `alpha-early` preserves the Bevy alpha |

Explicitly **not** goals (recorded to prevent scope creep in agent sessions):

- No currency-dependent core loop, no violence-required gameplay (VISION §1).
- No generative-AI NPC simulation on the server (VISION §4).
- No live ground physics from orbit — parked records (VISION §7 / architecture).
- No separate globe renderer, no loading screens between bodies (VISION §3).
- No further Bevy feature development; the alpha is frozen on `alpha-early`.
