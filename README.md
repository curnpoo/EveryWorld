# EveryWorld

**Drop anywhere on real Earth. Walk, rocket-jump, skydive, drive, fly — and launch to the Moon.**

EveryWorld builds a real, playable planet from public data: OpenStreetMap, Copernicus/Mapterhorn elevation, Open-Meteo weather, and more. The data decides *what* is there; procedural, region-aware rules decide *how it looks*. The result is a world that feels like *that place* — Zermatt’s alpine valleys, Shibuya’s neon canyons, Manhattan’s grid — all generated on the fly.

> Rust + Bevy. Native Metal renderer on macOS; Bevy/wgpu (D3D12 or Vulkan) on Windows.

---

## Highlights

- **Real Earth, not a level.** Slippy tiles, real elevation, real roads, real buildings, real weather.
- **Region-aware world generation.** Architecture, street kits, vegetation and ground cover change with climate and culture.
- **Movement that goes vertical.** Walk, rocket-boots, skydive, parachute, wingsuit, Mach-1 boom.
- **Built-in UI map.** Pick a drop point on the globe, choose your time of day, fall in.
- **Native Metal renderer (macOS).** HDR, GGX PBR, GTAO, SSR, bloom, cascaded shadows, probe GI, ocean FFT.
- **Deterministic.** Same seed + data version + UTC time = the same world for everyone.
- **Headless tooling.** `--perf`, `--screenshot`, `--lookdev` for repeatable benches and art review.

---

## Quick start

```bash
cargo run --release
```

The game opens into its own UI/map — choose where to drop in and go. Location flags are optional overrides (handy for scripting/tests):

```bash
cargo run --release -- --lat 43.53 --lon 5.45 --alt 3000   # skydive into specific coords
cargo run --release -- --lookdev lookdev/                   # render art-direction boards
```

---

## Setup

### macOS

1. Install Rust:
   ```bash
   brew install rust
   ```
   or via rustup: `curl https://sh.rustup.rs -sSf | sh`
2. (Optional) Xcode Command Line Tools:
   ```bash
   xcode-select --install
   ```
3. Build and run:
   ```bash
   cargo run --release
   ```

### Windows

1. Install Rust (MSVC toolchain) from <https://rustup.rs>.
2. Install **Visual Studio Build Tools 2022** with the *Desktop development with C++* workload.
3. Build and run in PowerShell:
   ```powershell
   cargo run --release
   ```

Full Windows details, test batteries and perf harness: [docs/WINDOWS.md](docs/WINDOWS.md).

> On Windows the game uses Bevy/wgpu (D3D12/Vulkan). The native Metal renderer is macOS-only.

---

## Controls

| Action | Input |
|---|---|
| Look | Mouse drag |
| Move | `WASD` |
| Up / down | `Q` / `E` |
| Fast | `Shift` |
| Speed | Mouse wheel |
| Time of day | `[` / `]` |
| Pause / menu | `Esc` |

---

## Optional CLI overrides

Mostly for scripting, perf runs and agents — the UI covers normal play.

```bash
cargo run --release -- --fly --lat .. --lon .. --alt .. --heading .. --pitch .. --hour 15
cargo run --release -- --perf                              # writes perf.csv
cargo run --release -- --screenshot out.png --frames 60
```

---

## Performance & tuning

- Headless perf runs: `tools/perf_headless.sh`
- In-game stats: `$env:EW_GFX = "stats=1"` (Windows) / `EW_GFX=stats=1` (macOS)
- Native Metal switches: `EW_NATIVE_NOFFT=1`, `EW_NATIVE_NOAO=1`, `EW_NATIVE_NOBLOOM=1`, `EW_NATIVE_NOSHADOW=1`, `EW_PROBE_GI=0`, …
- Full state + open work: [docs/HANDOFF.md](docs/HANDOFF.md)

---

## Multiplayer (ghost / testing phase)

Run a relay server on one machine, then connect clients on the LAN.

**Server (host machine):**
```bash
cargo run --release -p ed_server -- 7777
```

**Clients (each machine):**
```bash
cargo run --release -p ed_game -- --server <host-lan-ip>:7777 --name <your-name>
```

Then in the game: `Esc` → **Multiplayer** tab → **Connect**. The first remote player's state teleports you next to them; others show as a blue capsule with a nametag (or a ground ring when far).

---

## Roadmap (summary)

Current stage: **A — Stabilize and measure.** Full detail: [docs/ROADMAP.md](docs/ROADMAP.md). User goals list: [docs/GOALS.md](docs/GOALS.md).

| Stage | Name | Key work | Status |
|---|---|---|---|
| **A** | Stabilize & measure | Bug fixes (B1–B12), Mac test pass, GPU perf measurement, perf gate, determinism CI, pinned data versions | **Now** |
| **B** | Fast | Finish M1 (GPU-resident tiles), M2 (`ed_gpu` + `ed_render` v1), 60 fps on 6 reference scenes | Next |
| **C** | Together | Ghost multiplayer hardening, `ed_store` world deltas, deterministic procedural climate | After B |
| **D** | Native & Nanite-like | Native Metal backend, cluster LOD + software raster, material/shader system v2, 120 fps | Overlaps |
| **E** | Lighting | GTAO, froxel fog, motion blur, night lights, virtual shadow maps, DDGI | Overlaps |
| **F** | Play | Helicopters/rockets/spaceships/boats, missions, NPCs/traffic/parked cars, animals, customization, coupled ocean | Overlaps |
| **G** | Space | Seamless ground→orbit→Moon, orbital clouds/atmosphere, explorable Moon/Mars, starfield, procedural planets/galaxy | Overlaps |
| **H** | Platforms | Windows (D3D12, DLSS/FSR3, RTX), mobile (iOS then Android) | Later |

Headline goals: a fast, beautiful, shared real-Earth world; renderer end state is native Metal + a Nanite-like geometry system (cluster LOD, GPU culling, visibility buffer, software raster) on `ed_gpu`/`ed_render`; multiplayer-safe generation (pure function of world seed + pinned data version + UTC time); every feature declares cost per tier with an off switch.

## Docs

- [docs/ROADMAP.md](docs/ROADMAP.md) — stages, status, what’s next
- [docs/CHANGES.md](docs/CHANGES.md) — recent changes and open checks
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — how it works
- [docs/BUGS.md](docs/BUGS.md) — known visual/feel issues
- [docs/plans/](docs/plans/) — engine, buildings, shadows, flora, and more
- [docs/solarpunk/](docs/solarpunk/) — Solarpunk World modular asset spec + reference images

---

## Data attribution

© OpenStreetMap contributors (ODbL), OpenMapTiles, OpenFreeMap, © Mapterhorn (Copernicus DEM, swisstopo, USGS 3DEP and others), AWS Terrain Tiles, Open-Meteo (CC BY 4.0). Tiles are cached locally; weather has a TTL.
