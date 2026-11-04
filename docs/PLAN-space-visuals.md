# Plan: space visuals (atmosphere from orbit, stars, Moon, planets)

Status: partly built. Weather/clouds come from `ed_climate` (`PLAN-world-sim.md`); perf is `PLAN-perf.md`.

## Where we are
There are two separate renderers, and they share almost nothing:

| | Map globe (`ed_game/src/globe.rs`, render layer 1) | In-game (`sky.rs`, streaming terrain) |
|---|---|---|
| Earth | unit sphere, EOX Sentinel-2 quadtree tiles, roads/borders burned in (`maprender::overlay_on_imagery`) | streamed terrain tiles + sea-level `OceanGlobe` above 30 km |
| Atmosphere | ray-marched shell `globe_atmo.wesl` (additive, alpha 0) | Bevy physical atmosphere + aerial perspective; built for the ground |
| Stars | procedural `starfield.rs` sky sphere (Milky Way band, ECI-aligned) | **none** |
| Sun/Moon/planets | true positions (`ed_space`) | true positions (`CelestialBody`) |
| Clouds | none | volumetric `clouds.rs` near the camera, live weather |

## Goals
1. Seamless climb: ground -> stratosphere -> orbit -> Moon without a visual "switch".
2. One look: the map globe and the in-game view from orbit should match (same atmosphere colours, same clouds, same stars).
3. Night side: city lights, terminator, moonlit clouds.

## Work items (in order)
1. **Stars in game**: reuse `starfield::generate` as a sky-sphere cubemap/equirect on the main camera. Fade by sky luminance (invisible by day at the ground, full above ~60 km or at night). Rotate with sidereal time, the same ECI frame as the globe.
2. **Star catalog**: replace random stars with the Yale Bright Star Catalog (~9k stars, tiny, public domain) for named constellations, keeping procedural fill for faint stars and the Milky Way. Hipparcos/Gaia only if needed.
3. **Atmosphere from orbit**: above ~50 km, the Bevy atmosphere is tuned for inside-the-air views. Add the `globe_atmo.wesl` shell (scaled to real Earth radius, f64-safe: march in Earth-centred units) as an outer limb layer, and crossfade from Bevy's aerial perspective to it between 30 and 100 km. Calibrate colours so both agree at the crossover.
4. **Far Earth surface**: above ~200 km, streaming terrain gets too coarse/expensive; draw the globe's satellite tile sphere at true scale in the main scene (shared tile cache with the map) and hide terrain beyond the horizon ring.
5. **Clouds from orbit**: a 2D cloud-cover texture on a shell at ~8 km, generated from `ed_climate` (cover + storm records: hurricane spirals). The same texture feeds the map overlay and the title globe; the volumetric `clouds.rs` takes over near the camera, seeded from the same field so they line up.
6. **City lights**: NASA Black Marble (VIIRS) tiles on the night side of both globes, masked by the sun terminator, dimmed under clouds.
7. **Moon**: procedural cratered terrain via a `TerrainGrid`-like provider on a 1737 km sphere, then NASA LOLA elevation tiles. No atmosphere, hard shadows, Earth-shine. Needs the floating origin to re-anchor to the Moon's frame.
8. **Planets**: textured spheres at true positions with correct phase; Mars MOLA terrain later (ROADMAP 3.4).
9. **Polish**: sun glare/lens flare in space, eclipse shadows (Moon on Earth, Earth on Moon), exposure auto-adjust between day ground and space.

## Constraints
- All far math in f64 (ECEF/ECI); render only relative to the floating origin.
- MSAA stays off; meshlets only for terrain/buildings, not the sky shells.
- `AlphaMode::Add` is premultiplied in Bevy: additive custom shaders must write alpha 0.

## Open questions
- One shared `GlobeTiles` cache resource for map + in-game far Earth, or keep them separate?
- Cloud texture resolution vs. `ed_climate` grid (1-2 degrees is too coarse to look like clouds; needs procedural detail on top).
