# EveryWorld (working title)

**A persistent solarpunk universe on a twin of the real Earth.** Walk, drive, fly and launch
to the Moon with no loading screens; see cities from far away and the Earth from orbit; earn
new tech through rank, trust and merit; build a home; come back to the car you parked.

Built on a custom Rust engine with its own editor.

## Start here

**[docs/MASTER.md](docs/MASTER.md)** is the overview and the map of all documents.

| Doc | What |
|---|---|
| [docs/VISION.md](docs/VISION.md) | The goals, in the owner's words |
| [docs/DECISIONS.md](docs/DECISIONS.md) | Every locked decision, in one list |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Target design |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Phases and gates |
| [docs/phases/phase-1.md](docs/phases/phase-1.md) | Current phase |
| [docs/phases/phase-0.md](docs/phases/phase-0.md) | Completed: exit record and carried items |
| [docs/reference/alpha-baseline.md](docs/reference/alpha-baseline.md) | The alpha's measured performance and the budgets set from it |
| [docs/plans/path-to-solo.md](docs/plans/path-to-solo.md) | Milestones from here to the playable solo loop |
| [docs/plans/](docs/plans/) | System briefs: world, renderer, streets, editor, water, gameplay, alpha migration |
| [docs/data/](docs/data/README.md) | Data sources and licensing |
| [docs/solarpunk/](docs/solarpunk/) | Asset and character specifications |
| [docs/alpha/](docs/alpha/README.md) | Archived alpha docs |
| [docs/CHANGES.md](docs/CHANGES.md) | Change log |

## Status

**Phase 0 — Prepare** is complete (exit record: [docs/phases/phase-0.md](docs/phases/phase-0.md)
§5). **Phase 1 — Engine base and editor shell** is next. Plans carry gates, not dates.

| Branch | Contents |
|---|---|
| `main` | The new engine and its docs |
| `alpha-early` | The frozen Bevy/Avian alpha: real-Earth streaming, robot courier, cars and planes, native Metal renderer, ghost multiplayer. Crash fixes only |

## Technical direction

Custom Rust engine · own GPU interface (Metal first, D3D12/Vulkan after) · cube-sphere
planet with one node address · a ladder of representations from building parts to night
lights for far-field views · chunk streaming from a globe pack, a tile service and local
generation · GPU-driven instance rendering with a slot for cluster geometry · editor mode
with override layers · streets from Overture and OpenStreetMap · Box3D physics ·
region servers over QUIC. Details: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Running the alpha

```bash
git switch alpha-early
```

```bash
cargo run --release
```

```bash
cargo run --release -- --lat 43.53 --lon 5.45 --alt 3000
```

```bash
cargo test --workspace
```

macOS needs Rust and the Xcode command-line tools. Windows setup is in
[docs/alpha/WINDOWS.md](docs/alpha/WINDOWS.md). Renderer notes:
[docs/alpha/HANDOFF.md](docs/alpha/HANDOFF.md).

## Data and licences

Alpha data: © OpenStreetMap contributors (ODbL), OpenMapTiles, OpenFreeMap, Mapterhorn
(Copernicus DEM and national models), AWS Terrain Tiles, Open-Meteo. Two alpha datasets
(WorldClim, the EOX Sentinel-2 mosaic) cannot ship and are being replaced; the new engine's
sources and rules are in [docs/data/README.md](docs/data/README.md).

Third-party code and assets: Box3D (MIT), meshoptimizer (MIT), Quixel Megascans (Fab
Standard License; assets ship only inside the packaged game). Attribution is generated from
the dataset registry into `CREDITS.md`.
