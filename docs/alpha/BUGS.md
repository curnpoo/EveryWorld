# Bugs

> **STATUS (2026-11-08):** with the custom-engine reboot (`docs/VISION.md`,
> `docs/plans/rearchitecture.md`), these alpha issues are **replacement criteria**, not a
> fix list. Do not fix visual/feel bugs on the Bevy renderer; each item below must be shown
> fixed *by construction* as the corresponding new subsystem lands (planet renderer,
> cluster-LOD foliage, roads v2, camera/sim loop). Crash fixes only, on `alpha-early`.

Open visual and feel issues reported from play on the Mac, 2026-11-04 (Central Park spawn, rocket shoes). Newest reports at the bottom of each group. When one is fixed, delete it here and note the fix in `CHANGES.md`. Screenshot sources are the reporter's Desktop captures from 5:29-5:33 PM that day (not in the repo).

Priority: **P1** breaks the look or feel on first contact, **P2** clearly wrong, **P3** polish.

## Streaming / LOD

| # | Pri | Issue | Notes / suspected area |
|---|---|---|---|
| B1 | P2 | **Low-res flash on tile refine.** Zooming in (descending), when a tile swaps to a more detailed one, the low-res version visibly flashes. Needs a smooth transition on zoom. | Hole-free swap exists (`streaming.rs`), but there is no cross-fade: swap is a hard pop, and the satellite layer (`ground_material::SatPool`) may show its low-res texels or default for a frame. Want dithered/alpha cross-fade or hold the parent until the child's sat layer is resident. |
| B2 | P1 | **Tree impostors flicker a lot.** FIX LANDED, needs Mac check (see `CHANGES.md`): 4-view blend, wider thinning ramp, no sub-2 px quads. | `impostors.rs` + `shaders/impostor.wesl`: distance thinning ("survivors grow", `GROW_MAX`, `MIN_PX`) likely toggles per frame/pixel; also check atlas view-index switching (hemi-octahedral) and TAA jitter interaction. Make thinning stable (hash by tree id, hysteresis) and blend atlas frames. |

## Shadows

| # | Pri | Issue | Notes / suspected area |
|---|---|---|---|
| B3 | P1 | STAGE 1 LANDED, needs Mac check (see `CHANGES.md`, plan `plans/shadows.md`): 4 AGL-planned cascades with ~5.5x even splits to 2.5 km walking / 12 km flying, far fade instead of the cut. Still open: shadows past that range (stage 2 horizon maps), towers > 750 m cast nothing (`SHADOW_EXPLODE_M`). **Shadow cascades are very irregular and hard-cut.** Very high quality only very close, low quality farther out, then a hard stop where shadows end entirely (screenshot 1: Central Park lawn, shadow edges are blocky/stepped and shadows vanish at a distance). | Cascade config in `sky.rs` (count, bounds, first cascade far, max distance). Need more even split (log/PSSM blend), larger max distance (or fade-out instead of cut), cascade blending, and a stable texel size. Remember the shadow proxies for clouds. |
| B4 | P2 | FIX LANDED for the ground, needs Mac check (roughness floor and distance fade in `terrain.wesl`); tree lighting not looked at. **Ground is shiny/specular in sun, trees are not lit the same** (screenshot 2: sunlit ground glints while tree canopies stay dark). Real ground is far less shiny. | Terrain material roughness too low / specular on satellite colour; raise roughness, drop reflectance for grass/dirt. Trees: check leaf cards/full trees receive sun (normals, shadow bias, translucency) so they are not dark relative to ground. |

## Roads and paths

| # | Pri | Issue | Notes / suspected area |
|---|---|---|---|
| B5 | P1 | **Roads are flat planes with no markings.** No lane lines, crosswalks, edges, or curbs. | `ed_world/src/roads.rs`. Add lane/centre-line markings by road class (procedural texture or geometry strips), sidewalks/curbs for urban, thickness, and drape on the `TerrainGrid` so they do not float or z-fight. |
| B6 | P1 | **Roads are not on the ground** (floating or not conforming to terrain). | Check road mesh height sampling against `TerrainGrid::sample` and that terrain may render at a coarser LOD than the grid (known open question in CLAUDE.md). Add a small bias plus a skirt, or sample on subdivided segments. |
| B7 | P1 | **Guardrails are not connected to anything** and are flat planes. | Give them posts, a W-beam rail profile with thickness, and attach to the road edge on terrain height; no free-floating segments. |
| B8 | P2 | **Dirt paths look like floating brown lines.** | Paths/tracks need to blend into ground (decal-style splat into the ground texture, soft edges, noise, wheel ruts) instead of a ribbon mesh above the surface. |

## Atmosphere, ground at altitude, clouds

| # | Pri | Issue | Notes / suspected area |
|---|---|---|---|
| B9 | P1 | **Atmosphere transition is rough.** Flying up, the atmosphere suddenly gets bluer, not smooth (screenshots 3-4). | `sky.rs` atmosphere params / scale height vs altitude; pop is likely a discrete switch (pipeline key, env-map refresh threshold, fog) rather than a continuous blend. Drive all atmosphere and fog parameters as continuous functions of altitude. |
| B10 | P1 | **Satellite imagery disappears at high altitude, leaving the procedural ground texture, which looks wrong** (screenshots 4-5: rectangular tile patches with different colours, blotchy greens/browns, hard tile borders). There is a certain altitude where ground texture shows but satellite does not. | `ground_material::SatPool` size classes / zoom selection at low-detail tiles: far tiles lack a sat layer and fall back to the splat/procedural look. Need a coarse global satellite or biome-coloured base that is continuous across tiles, no tile-shaped seams, and a smooth blend between sources. |
| B11 | P1 (native port done 2026-11-05, needs Mac check) | **Clouds look terrible at high altitude and high speed**: pixelated, very low quality (screenshots 4-5: cloud blobs are dotted/dithered grids). | `clouds.rs`, `cloud_half.rs`, `clouds.wesl`: half-res cloud march with dither, likely too few steps and no temporal reprojection/upsample at speed. Needs a proper bilateral upsample or TAA-stable accumulation, and an altitude-aware LOD (flat layer / billboards from above). |

## Camera

| # | Pri | Issue | Notes / suspected area |
|---|---|---|---|
| B12 | P1 | **Camera lags behind the character**, even at low speed when accelerating; at high speed follow does not work (screenshot 5: character visible far ahead/offset). | `camera.rs`. Desired: camera is positionally **locked** to the character (no spring lag on position). Keep a little motion on walk/jump (head bob, landing dip, small easing on the vertical axis). In flight, only light smoothing, and it must be velocity-compensated (smooth in the character's frame, or integrate camera in the same step after physics) so speed never produces a growing lag. Likely cause: smoothing in world space with a fixed time constant, or camera updated before the player transform. |

## Buildings

| # | Pri | Issue | Notes / suspected area |
|---|---|---|---|
| B13 | P1 | **NYC window frames render detached from facades** (floating frames in front of the walls at street level, native renderer; check if Bevy too). | Likely a transform/offset in the window geometry or its mirroring in `native.rs`; repro: `--fly --lat 40.7484 --lon -73.9857 --alt 60 --heading 200 --pitch -15 --hour 16`. |

## Cross-cutting notes

- B9, B10, B11 are all altitude-dependent transitions. Consider one `AltitudeBlend` resource (smooth 0..1 per regime) that atmosphere, sat/ground texture, fog and clouds all read, so regimes cannot pop independently.
- B3 and B4 interact (shadow range and terrain lighting): fix them together and re-shoot `tools/shoot_places.sh`.
- Repro for most: `cargo run --release -- --place "Central Park"`, walk, then rocket up with boost and descend.
