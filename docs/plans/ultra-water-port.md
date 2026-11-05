# Porting Ultra-Water (three.js) to the native Metal renderer

Source: https://github.com/Humpalumps/Ultra-Water (MIT, (c) 2026 Humpalumps). Study date 2026-11-05. Keep its MIT notice if code is copied.
Read: README, `src/water/{OceanSim,OceanSurface,shoreMap,Caustics,UnderwaterPass,WaterQuery,bakeTextures}.js`, `src/scene/{islandHeight,floaters}.js`.
Not read: PostFX, PlanarReflection, sky/atmosphere, Spray, MarineSnow. The repo wiki is empty. All its shaders are inline strings in the JS.

## What it is
Tessendorf FFT ocean: 4 cascades (tile 600/85/12/2.1 m, 256^2 each, JONSWAP wind sea + swell, capillary band), Jacobian foam, a baked shore field
(travel-time eikonal + Green's-law shoaling + breaker model), area-ratio caustics, underwater pass, GPU height query for buoyancy.
Reported cost: 3.3-5.9 ms on an RTX 3060 at 3.3 MP, FFT sim about 0.45 ms (21 draws).

## Feature -> EveryWorld mapping

| Feature | How it works there | Port to ed_metal | Effort | Notes |
|---|---|---|---|---|
| FFT waves, 4 cascades | JONSWAP h0 init, evolve, 16 Stockham radix-2 stages, assemble. Output displacement (lambda*Dx,Dy,lambda*Dz,turb) + slopes, RGBA16F, mipmapped | Metal **compute** kernels (init once, evolve+FFT+assemble per frame). Same math, no MRT hacks | M | 3 gravity cascades is enough; keep the 300 s loop-quantised time so fp32 never drifts |
| Surface mesh | camera-centred polar grid, ~190k verts, LOD by `log2(cell*N/L)` | Replace the terrain water mask for near water with this mesh; far water keeps analytic normals | M | Our water is currently a terrain mask. Needs its own mesh draw pass |
| Shading (GGX sun, Fresnel F0=0.02, roughness from slope variance, Cox-Munk remainder, geometric AA) | formulas in report below | Port as-is into a `water_fs` | S | Slope variance from the 1x1 mip of each cascade |
| Sun glitter (Poisson facets) | per-pixel-cell Poisson draw, 2e4 facets/m^2 | Port | S | The sparkle people notice |
| SSS on crests, Beer-Lambert absorption, scatter colour | `k1 = 1.6*h*pow(sun.-V,4)*pow(.5-.5NoL,3)`, absorption (0.40,0.062,0.032)/m | Port | S | Presets: clear/tropical/murky are just constants |
| Reflection | half-res planar + sky cube | We already have SSR + analytic sky. Add a **planar reflection pass** at half res for water only, SSR as fallback | M | Fixes the cut-off SSR: planar has no screen edge |
| Refraction + depth validation | offset by normal, validated against scene depth, fallback to unrefracted | Port using our linear depth texture | S | Needs opaque scene colour resolved before the water pass (we have HDR) |
| Foam (Jacobian + baked lace texture + shoreward flow) | turbulence from Jacobian with recovery; 512^2 Voronoi/billow bake; flow-map advection | Port; bake the foam texture once at startup on CPU | M | |
| Caustics | area-ratio splat of refracted grid from the 12 m cascade into 3 depth slices | Port as 3 small render passes, sample on seabed/terrain under water | M | Needs underwater terrain to be lit; we have terrain under the mask |
| Shallows / coastline | CPU-baked 512^2 shore map: signed coast distance, travel time tau, direction, shoal factor, shelter depth S. Breakers phase = omega*(tau+t), amplitude capped 0.42*depth, peeling noise | Bake **per tile** from our land/water mask (OSM coast + satellite). Depth is unknown, so synthesise a shelf: `depth = f(distance to shore)` | L | The one real research gap, see below |
| Beach swash/wet sand | shared GLSL for water and terrain | Apply to terrain shader near coast (`wet` already exists in terrain: `in.col.a`) | S | |
| Underwater view | Snell's window, TIR, absorption fog, lens depth, light shafts | Port when camera dips below the surface | M | Low priority for a flying game |
| Buoyancy / floaters | **GPU height query**: 512 points evaluated with the exact rendered surface, async readback 1-3 frames late; per-cell buoyancy 1025*g*vol*sub, drag, 240 Hz substeps | Do NOT copy: see Determinism below | M | |
| Spray, marine snow, lens droplets | particles / post | Later; boots plume system could host spray | S | |
| Atmosphere (Hillaire), AgX, auto-exposure | separate from water | Skip; we have our own | - | |

## Open problems specific to EveryWorld
1. **No bathymetry.** Ultra-Water has an analytic island with true depth. We only have a coverage mask. Plan: from the land/water mask compute a signed distance to coast per tile (EDT), then `depth = clamp(dist_from_coast * slope, 0.3, 14)` gives shoaling, breaking and shallow turquoise for free. Deep ocean = depth >= 14. Real bathymetry (GEBCO) could replace it later.
2. **Planet scale.** The FFT tiles are metres; we render a round Earth up to 20 km. Water mesh must be camera-centred and in the local tangent frame, with the Earth-curvature drop applied after displacement.
3. **Determinism / multiplayer.** A GPU FFT is not bit-identical across machines, so buoyancy must not read it back. Use a CPU-evaluable wave model for physics: a fixed set of about 16 Gerstner/linear waves sampled from the **same JONSWAP spectrum** (seeded, same phases), identical on every client. Render uses the FFT; physics uses the analytic sum; they agree to within a few cm for the large waves that move boats. Floating props then query the CPU function, no readback.
4. **Sky/sun coupling.** Water must reflect our `sky_rad` and clouds; the planar pass has to render the same sky.
5. **Coverage mask in the renderer.** Terrain currently draws water via `water_f` (satellite alpha). The new water mesh should replace it where `water_f` is high and handle the blend at the coast (foam line).

## Proposed phases
1. (done, committed with this doc) wave normals + analytic sky reflection fallback in `world.metal`.
2. Shading port: Fresnel, GGX roughness from slope variance, Beer-Lambert colour with depth-from-shore, SSS, glitter, planar reflection. Biggest visual win.
3. Metal compute FFT, 3 cascades, displacement + slope + foam turbulence. Water mesh pass near camera.
4. Coast: per-tile shore field, shoaling, breakers, foam lace texture, swash on terrain.
5. Refraction, caustics, seabed.
6. CPU wave model for buoyancy + floaters API (`ed_physics`), spray particles.
7. Underwater pass.

## Budget
Target 1.5-2.5 ms total on M3 Pro at 1440p (FFT 0.4, surface 1.0, caustics 0.2, planar 0.5 when water is visible). Skip FFT/caustics when no water is on screen.

## Verify
`earthdrop --headless --fly --lat 37.80 --lon -122.46 --alt 40 --heading 60 --pitch -8 --hour 16 ...` (SF Bay), open ocean (lat 36 lon -123.5), a beach (Santa Cruz 36.96,-122.02).
The interactive reference page of four shader styles was published as an Artifact (EveryWorld Water Options).
