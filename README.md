# EveryWorld

Drop anywhere on the real Earth. Walk, rocket-jump, skydive, drive, fly, and launch to the Moon.
Built from public map data (OpenStreetMap via OpenMapTiles, Mapterhorn/Copernicus elevation, Open-Meteo weather) and made to look like *that place* with procedural, region-aware rules.

Rust + Bevy, renders with Metal on macOS.

```bash
cargo run --release -- --place "Zermatt"
cargo run --release -- --lat 43.53 --lon 5.45 --alt 3000      # skydive into Provence
cargo run --release -- --place "Shibuya" --hour 18.5          # golden hour in Tokyo
cargo run --release -- --lookdev lookdev/                     # render art-direction boards
```

Controls (fly cam for now): mouse drag to look, WASD move, Q/E down/up, Shift fast, scroll to change speed, `[` `]` scrub time of day.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how it works and [docs/ROADMAP.md](docs/ROADMAP.md) for what's next.

## Data attribution

© OpenStreetMap contributors (ODbL), OpenMapTiles, OpenFreeMap, © Mapterhorn (Copernicus DEM, swisstopo, USGS 3DEP and others), AWS Terrain Tiles, Open-Meteo (CC BY 4.0).
