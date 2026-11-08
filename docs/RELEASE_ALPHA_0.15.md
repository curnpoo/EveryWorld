# EveryWorld — Alpha 0.15

**Real Earth. Drop anywhere. Walk, drive, fly, rocket.**

Alpha 0.15 is the first public-ish build of EveryWorld: a procedural, real-data Earth you can drop into anywhere on the planet. It runs on macOS today (a `.app` is attached). Windows is **not** in a usable state yet (see below) — you can build it yourself, but expect breakage.

> This is an alpha. Expect low frame rates in cities, visual glitches, and features that are half-finished. Please report issues with your hardware + the place you dropped into.

---

## What works

- **Real Earth, real data.** Terrain (Mapterhorn/AWS elevation), buildings/roads/water from OpenFreeMap, land cover (ESA WorldCover), climate-driven biomes, real sun position and day/night.
- **Procedural world.** Per-region architecture (17 regions + street kits), modeled building facades, trees with wind, ground scatter (grass, pebbles, moss), rocks, street furniture.
- **Movement.** Walk/run, skydive, parachute, wingsuit, rocket boots, Mach-1 boom. First and third person robot courier.
- **Vehicles.** Cars (raycast suspension) and planes are playable prototypes.
- **Atmosphere & sky.** Physically based sky, sunsets, aerial perspective, Moon and planets at true positions, volumetric clouds driven by live weather.
- **Native Metal renderer (macOS).** HDR, SSAO, TAA, bloom, Blender Filmic tone mapping, cascaded shadows, water.
- **Streaming world.** Quadtree LOD tiles stream in around you; floating origin keeps precision from a pebble to the stratosphere.
- **Map & UI.** Place-name search, satellite globe drop-in picker, minimap, compass, HUD stats, pause menu (Map/Settings/Character/Controls), time slider.
- **Missions.** Delivery missions planned over real airports/spaceports.
- **Multiplayer (ghost).** A working ghost-multiplayer baseline: remote avatars with shared IK rig, wing deformation, flames. Not persistent yet.
- **Audio.** Ported engine-sound kit (engine voices, jet/steam, 31 vehicles).

## What doesn't work / known limitations

- **Windows is broken right now.** The Bevy/wgpu dev path exists and CI builds it, but the native renderer, streaming, and several features are macOS-first. A Windows `.exe` can be built (command below) but expect crashes, missing features, and poor performance. A native D3D12 backend (DLSS/FSR3, RTX) is planned but not built.
- **Performance is below target.** Target is 60 fps; an M3 Pro sits around 24–35 fps in cities (worse while streaming). Terrain tiles can hitch (~1 s freeze when a tile arrives). Needs decent hardware.
- **High altitude / space is unfinished.** Terrain disappears when you go high and precision degrades; there is no seamless ground↔orbit transition yet. Clouds, atmosphere and shadows are tuned for ground views. You can fly high, but it won't look right above ~a few km.
- **No vehicle persistence.** Park a car, fly away, and it's gone when you come back (tiles unload). "Parked state" persistence is planned.
- **Moon/Mars are sky objects only.** Not explorable surfaces yet. Procedural planets and galaxies are planned.
- **Weather is live (Open-Meteo), not yet deterministic.** Multiplayer world-match is in progress.
- **Building interiors, bridges/tunnels, traffic/NPCs, animals, character customization** are not built yet.
- **Art direction round 2** not reviewed; some LOD pop-in, impostor flicker, shadow-cascade glitches.
- **Mobile (iOS/Android)** not supported.

## Hardware

- **macOS:** Apple Silicon (M1/M2/M3) recommended, 16 GB RAM. M3 Pro tested. macOS 13+. Metal required.
- **Windows:** untested in practice; needs a capable GPU. Treat as experimental.

## How to play

### macOS (.app attached)

1. Download `EveryWorld.app` (attached to this release).
2. Move it to `/Applications` (or anywhere).
3. Right-click → Open the first time (unsigned build).
4. Choose a drop-in from the menu, or run from terminal:

```bash
/Applications/EveryWorld.app/Contents/MacOS/earthdrop --lat 43.53 --lon 5.45 --alt 3000
```

### Windows (build from source)

Rust toolchain required (`rustup`). From a PowerShell prompt:

```powershell
git clone https://github.com/<you>/EveryWorld.git
cd EveryWorld
cargo build --release
```

The executable is at:

```
target\release\earthdrop.exe
```

Run it:

```powershell
.\target\release\earthdrop.exe
```

Optional: skydive into a location:

```powershell
.\target\release\earthdrop.exe --lat 43.53 --lon 5.45 --alt 3000
```

Run tests:

```powershell
cargo test --workspace
```

### Controls (short)

- WASD move, mouse look, Space jump/rocket boots, Shift sprint.
- E to enter/exit vehicles. V toggle vehicle camera.
- Esc pause menu (Map / Settings / Character / Controls).
- Type a place name in the menu map to drop in anywhere.

### Useful flags

- `--fly` spectator camera
- `--perf` writes `perf.csv`
- `--screenshot out.png --frames 60`
- `--no-post`, `--no-atmosphere`, `--scale 0.67`
- Env: `EW_NATIVE=0` (use Bevy renderer), `EW_PERF=1`

## Feedback

File issues at the repo's issue tracker. Include: macOS/Windows version, GPU, the coordinates you dropped into (`--lat/--lon`), and a screenshot if visual. For performance reports, attach `perf.csv` from a `--perf` run.

---

*EveryWorld is built on free data (Mapterhorn, OpenFreeMap, ESA WorldCover, Open-Meteo) and open-source Rust. See `AGENTS.md` / `docs/ARCHITECTURE.md` for how it's built.*