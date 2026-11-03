# Clouds v2

Renders here come from `earthdrop --lookdev DIR --only clouds --width 1280 --height 720 --cloud-quality high`
(software Vulkan, so no real timings). Shots: fair cumulus, overcast stratus, distant thunderstorm, sunset (front lit and backlit),
above the deck at 4,000 m, and cloud shadows on the ground.

## Technique
- Full-screen raymarch (`crates/ed_game/src/clouds.wesl`) of a spherical cloud shell, drawn as a premultiplied transparent
  material after the Bevy `Atmosphere`. It reads the depth prepass, so terrain and buildings occlude clouds.
- Shapes: tileable Perlin-Worley and Worley fBm 3D textures (`ed_world::clouds`, 128^3 shape, 64^3 detail, built at startup in ~1.7 s on 4 cores).
- Cumulus (flat base, domed top, coverage clumping), a stratus deck, and cumulonimbus cells with a leaning tower, flared anvil and an analytic rain shaft.
- Lighting: sun through an analytic atmosphere (warm and dim when low, Earth shadow), dual-lobe phase for silver linings,
  4 multiple-scattering octaves, powder term, sky and ground ambient for blue-grey bellies, horizon haze.
- Ground shadows: a camera-following caster plane on a render layer only the sun sees. Its prepass shader discards where the
  column of cloud toward the sun is thin. Stratus decks instead dim the sun globally (`CloudSunFactor`).
- Weather: `CloudParams::from_conditions` maps WMO code, cover and wind to coverage, heights, density, storminess and rain.
  The hourly forecast is interpolated while the clock timelapses. Thunderstorm codes spawn storm cells on a geodetic grid that drifts with the wind.
- Floating origin: noise is sampled at ECEF minus wind drift, wrapped modulo 262,144 m in f64, then offset in f32 by the render-space ray.
  All texture periods divide the wrap, and a unit test checks two frames 8+ km apart agree.
- Flying high: above the deck the march enters the top of the shell and stops at the bottom (see the 4,000 m shot).

## Flags
`--weather clear|fair|overcast|rain|showers|storm|fog` (also skips the network), `--cloud-quality low|medium|high`
(low is auto picked on software renderers), `--no-clouds`.

## Apple Silicon expectations (not measured)
High quality is 160 steps with 6 light steps, which should be several ms per frame at 1440p on M-series. Medium or low
are the fallbacks. Not benchmarked on a Mac yet, so please check frame time.

## Known gaps
- Real-place check at 17:00 in November is after sunset in NYC, so it is dark. Try `--hour 15`. The NYC render was not captured in CI (tile streaming takes ~12 min on CPU).
- Cloud shadow map uses a 3 km cascade range, so shadows far from the camera are not drawn.
- Rain is a shaft effect only, no particles, no splashes, no sun glare on wet ground.
- Sky ambient for clouds is a parametric model, not read from the atmosphere LUTs, so twilight color can drift from the sky.
- Cloud layer heights are fixed per weather type, not from real altitude data. No fly-through fog inside clouds beyond the march itself.
- Low sun angles can show banding with TAA at low quality.
