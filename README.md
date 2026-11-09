# EveryWorld (working title — final name TBD)

**A persistent solarpunk universe. Photoreal Earth beneath your feet, cel-shaded heroes,
friends in the next valley and on the Moon — and no loading screens between any of it.**

Explore a twin Earth that grew into solarpunk abundance: dense rewilded forests, green cities
you can drive with sim-grade handling, rocket boots, spaceports, walk-in buildings. Earn new
tech and transport through **ranks, trust and merit — not money**. Build a home base. Meet
NPCs who live persistent lives. Then climb into a rocket, watch the curved Earth fall away,
land on the Moon, and come back to the car you parked.

---

## Start here: **[docs/MASTER.md](docs/MASTER.md)**

The master plan contains everything: vision and goals (owner's verbatim words), the audit,
full target architecture, systems and implementation, data registry and licensing, art
direction with reference imagery, all phases with gates, acceptance metrics and risks.

More detail per topic:

| Doc | What |
|---|---|
| [docs/VISION.md](docs/VISION.md) | Goals, verbatim, each mapped to real systems |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Phases 0–8 and current status |
| [docs/phases/phase-0.md](docs/phases/phase-0.md) | Current phase in detail: why, workstreams, pass criteria, exit gate |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Target architecture |
| [docs/plans/rearchitecture.md](docs/plans/rearchitecture.md) | The audit + full technical plan |
| [docs/data/](docs/data/README.md) | Planetary data catalog, licensing, acquisition |
| [docs/solarpunk/](docs/solarpunk/) | Modular asset spec + character spec |
| [docs/alpha/](docs/alpha/README.md) | Archived alpha docs (bugs, handoff, old plans) |
| [docs/CHANGES.md](docs/CHANGES.md) | Dated change log |

## Status and pacing

- **Custom-engine reboot, Phase 0 (prepare).** All-in: no parallel Bevy development.
- **Multi-month on purpose:** roughly **6–10 months to the full vision**. The playable alpha
  was built in 4 days with AI coding models; the phase gates exist so that speed never
  strands the project unplayable.
- Handcrafted assets + guided procedural generation — the goal is an authored world, not an
  "all code" one.

### Branches

| Branch | Contents |
|---|---|
| `main` | The new custom engine (`ew` app, `ed_gpu`/`ed_render`, cube-sphere `ed_planet`, Box3D `ed_sim`, region-sharded servers) |
| `alpha-early` | The frozen playable Bevy/Avian alpha — real-Earth streaming, robot courier, cars/planes, native Metal renderer, ghost multiplayer. Crash fixes only |

### Tech direction (locked)

Custom Rust engine, own GPU RHI (Metal first, D3D12/Vulkan later) · cluster-LOD + visibility
buffer + voxel-clipmap GI (Solstice techniques, MIT) · cube-sphere planet with slippy tiles
as the data layer · Box3D deterministic physics (MIT) · region-sharded persistent servers on
Ubuntu LTS · Megascans (Fab Standard License) + authored solarpunk kit · VRM avatars later.

## Running the alpha (branch `alpha-early`)

```bash
git switch alpha-early
cargo run --release                                        # play (choose drop-in from the UI)
cargo run --release -- --lat 43.53 --lon 5.45 --alt 3000   # skydive into specific coords
cargo run --release -- --lookdev lookdev/                  # art-direction boards
cargo test --workspace                                     # fast, offline tests
```

macOS: Rust (+ Xcode CLT). Windows: Rust MSVC + VS Build Tools (C++ workload); see
[docs/alpha/WINDOWS.md](docs/alpha/WINDOWS.md). Alpha handoff/native-renderer notes:
[docs/alpha/HANDOFF.md](docs/alpha/HANDOFF.md).

Controls: mouse drag to look, `WASD` move, `Q/E` up/down, `Shift` fast, wheel speed,
`[`/`]` time of day, `Esc` menu.

Ghost-multiplayer test (alpha): `cargo run --release -p ed_server -- 7777` on the host, then
`cargo run --release -p ed_game -- --server <host-ip>:7777 --name <name>` per client.

## Data attribution and licensing

Alpha: © OpenStreetMap contributors (ODbL), OpenMapTiles, OpenFreeMap, © Mapterhorn
(Copernicus DEM, swisstopo, USGS 3DEP and others), AWS Terrain Tiles, Open-Meteo (CC BY 4.0).
**Migration in progress:** WorldClim (climate) and the EOX Sentinel-2 mosaic are not cleared
for commercial distribution and are being replaced — [docs/data/README.md](docs/data/README.md).

Third-party: Solstice (MIT, rendering techniques), Box3D (MIT, physics), Quixel Megascans
(Fab Standard License — no standalone asset redistribution; this repo is private and assets
ship only inside the packaged game). Attribution is generated from the dataset registry into
`CREDITS.md` and an in-game credits screen.
