# Changes

Recent work, newest first, and what still needs checking. When an entry is verified on the Mac, fold the result into `ROADMAP.md` and trim it here. Plans live in `docs/plans/`; the roadmap says what's done and what's next.

---

## 2026-11-05: controller menus, button prompts, analog triggers
- Controller menus: D-pad or left stick moves focus to the nearest button in that direction (`menu.rs` `pad_focus`), A selects, B goes back (resume when paused, main menu from a tab), LB/RB switch tabs. Focus auto-starts on the top button; the Map tab has none (crosshair is its cursor).
- Button prompts: a bottom strip of Xbox-style glyphs (A green, B red, pills for LB/RB/RT/LT/D-pad/Start) that changes per screen, in controller mode only (`gamepad.rs` `update_hints`).
- Analog triggers: RT travel scales rocket thrust, LT travel scales the afterburner boost (`Pad.rt/lt`, `Player.rocket_amt/boost_amt`). Keys stay full strength.
- Test hook: `EW_PAD_SCRIPT="30:down,60:a,90:rt=0.3,120:shot=/tmp/x.png"` injects pad input and screenshots (no controller needed).
- Check on a real pad: stick-flick menu stepping feel, thrust at light trigger pulls, glyph legibility in game (not screenshot-tested in Playing).

## 2026-11-04: quieter wind, nozzle-anchored boot jet, no beeps on spawn
- Wind: ear wind follows a capped log curve of q (about -10 dB at freefall speed); howl, suit flutter and world wind roughly halved; height shear capped lower (`ed_audio/src/scape.rs prepare`, `soundscape.rs`).
- Spawn beeping was the ground-proximity warning: any fall faster than 1 m/s with under 4 s to impact set it off, including the default 40 m spawn drop. GPW now needs > 20 m/s down and 4 s of sustained fall; the altimeter also needs Skydive mode and a fall that started above 1800 m AGL.
- Boot plume: particles inherit the full player velocity (were 8 %), so the jet stays on the nozzle and trails back at speed. Spread and sprite growth are tighter, and there are new per-nozzle core streaks (orange sheath + white core) that bend into the airflow and stretch up to 3.5x with airspeed.
- Fix (same day): fresh particles were placed one to two frames *ahead* of the nozzle (sub-frame jitter added the full inherited velocity, then the same frame advanced them again). Now they are advanced before spawning, and the jitter offset is only the exhaust part (`vel - world_vel`).
- Check on the Mac: hover, full Shift boost, fast level flight (jet should lengthen, not smear), and that the default spawn is silent.

## 2026-11-04: gamepad support, auto-detected (unverified: no controller tested)
- New `ed_game::gamepad`: one `Pad` snapshot per frame (sticks with a 0.18 deadzone, triggers, button edges) merged over all pads, and `InputMode` (Keyboard / Controller). The first stick or button input switches to Controller; the first key, mouse button or real mouse movement switches back.
- Gameplay: left stick = WASD, right stick = look (same invert setting and sensitivity as the mouse), RT = Space, LT = Shift, A = F (wingsuit), B = P (parachute), Start = Esc (pause and resume). Keyboard and pad inputs add together.
- Menus: LB / RB cycle the tabs (Map, Settings, Audio, Character, Controls) with the active tab lit. A presses Play on the main menu. Controller lines were added to the Controls tab.
- Map: left stick pans (speed scales with height, like a drag), right stick zooms toward the crosshair, and A picks the spot under a centre crosshair. With the drop popup open, A = Drop from the sky and B = Cancel. The popup is handled in `globe::control` so one A press can't both select and drop.
- D-pad moves a focus highlight through the visible menu buttons in spawn order (down/right next, up/left previous, wrapping). A presses the focused button, B resumes from the pause menu. Focus clears when the mouse or keyboard is used, and the drop popup takes A and B instead. Map A-select is skipped while a button is focused.
- In controller mode the mouse pointer hides on the menu screens. Sliders (audio, time of day) still need the mouse; the Start button also resumes.
- **Check on a real pad:** stick directions (up = forward / look up / pan north / zoom in), RT and LT thresholds, that Start doesn't fire twice, and that switching devices flips the crosshair and cursor both ways.

## 2026-11-04: audio settings tab + settings persist between sessions
- Pause/main menu has a new **Audio** tab with sliders for master, world ambience, footsteps/robot, wind/rockets, vehicle engines and alerts, plus Mute and Calm ambience toggles. Calm ambience is the `focus()` thinning; turn it off to hear every bed.
- `ed_audio::Params::mix` adds per-bus gains (body, air, amb, ui), smoothed like the other params, with a test. The master and vehicle sliders also drive `engine_audio` and `sonic_boom`. Master 70 % (`settings::MASTER_REF`) gives the tuned levels.
- `ed_game::settings` saves audio, graphics toggles/resolution, mouse settings and backpack to `~/Library/Application Support/EveryWorld/settings.json` (`EW_SETTINGS=path` overrides). The file is written atomically about 1 s after the last change and on exit.
- Automated runs (`--screenshot`, `--perf`/`EW_PERF`, `--headless`, `--lookdev`, `--quit-after`) only read the file. CLI graphics flags (`--no-post`, `--no-shadows`, `EW_GFX`) override the saved graphics without overwriting them. `EW_SOUND` still overrides master.
- **Check on the Mac:** drag the sliders while playing and confirm each one changes the right sounds, then quit and relaunch and confirm the values come back.

## 2026-11-04: procedural soundscape (`ed_audio`) + IceRunner boot plume
- New engine-free `ed_audio` crate (24 tests): all sound is synthesised, with no samples. Covers robot footsteps for 12 grounds, landings, jump charge, servos; q-driven wind, tuck vs spread, wingsuit, tree whooshes, chute; boot pops, flames, Shift spool, stage kicks, ground blast, space thrum; tyres, skid, thumps, stall horn; leaf wind per tree type, surf, rivers, waterfalls, rain per surface, thunder, animals by biome/time, Dolbear crickets; traffic, rail, canyon echo, 50/60 Hz hum; altimeter, GPW, re-entry.
- `ed_world::soundmap`: per-z14-tile distance fields (roads by class, rail, ocean, lake, river, waterfall) + building cover/height, built in `tilegen`, carried as `soundscape::TileSound`.
- `ed_game::soundscape` fills `Params` + edge events each frame. `focus()` keeps only the loudest ~2 beds and 2 animal groups up front (first listen was "so loud and busy"). `EW_SOUND=0..1` volume, default 0.35. Thunder is random during storm weather codes (no lightning system yet).
- `ed_game::boot_plume`: port of IceRunner `ExhaustVfx.ts` (soft noisy sprites, gold -> orange -> ember -> smoke, afterburner blend, warm light) on both boots; particles splash along the ground. Outer flame cone hidden, core kept.
- Check on the Mac: overall loudness and busyness in Central Park / Shibuya / Zermatt; plume size (`SIZE_K`, `SPEED_K`) and light intensity; footstep timing vs. the gait.

## 2026-11-04: near-chunk collider crash, road triangle cut, perf bisect knobs (Manhattan 16 -> ~18 fps)
- Crash: `near_chunks` built rock colliders with `Collider::trimesh_from_mesh` on a RENDER_WORLD-only mesh, which panics once the mesh is extracted. `Item::col_src` now keeps a CPU-only position+index copy for the collider. No leak found; the "gets worse" feel is geometry load.
- Roads (`roads.rs`): `simplify_deck` (3D Douglas-Peucker + max span) feeds `sweep`/`markings` (deck 0.08 m tol / 12 m span, ways 0.05 m / 6 m). Guardrails, props and centerlines keep the full line. ~50% fewer road triangles; Manhattan `vis_other` 28.6M -> 19.3M.
- Knob: `EW_HIDE=terrain,roads,lod,rocks,props,water` skips spawning those parts (bisecting only).
- Bisect at Manhattan z=250 m (ms/frame, noisy, load avg ~6): base ~56; no terrain 41; no roads 43; everything hidden 25. Post+shadows off (`EW_GFX=ao=0,bloom=0,aa=0,shadows=0`) 39. So ~25 ms is a floor independent of geometry, mostly post/shadows/sky. `EW_HIDE=lod` showed no change: check that it covers the real building path before trusting it.
- Next: terrain LOD at distance (301k tris per z14 tile), cheaper SSAO/bloom/shadow passes, M2 clipmap.

## 2026-11-04: plane-style flight, wingsuit by pose, rockets steer with the feet (unverified in-game)
- Controls (wingsuit and rocket shoes): S = nose up, W = nose down, A/D = bank/roll, mouse = where to go (the flyer turns toward the view by banking/swinging, not by snapping). Space = boost (1.5 g, was 2.4 g), Shift = afterburner, ramping slowly (`flyer::boost_ramp`, ~10 s) up to ~36 g.
- No invisible forces. Engine-free models in `ed_flight::flyer` (tested with a rigid-body sim):
  - Wingsuit: keys/view drive a `WingPose` (arch, twist) via `wing_pilot` (bank-to-turn toward the camera heading); lift/drag/pitch/roll moments come from that pose (`wingsuit`). Measured 30 s glides: dive 87 m/s L/D 0.6, neutral 44 m/s L/D 2.5, flare 30 m/s L/D 2.9. Rate gains capped (`MAX_RATE_GAIN`) because roll inertia is small at high q.
  - Rockets: `rocket_axis` (gravity-compensated aim) -> `rocket_attitude` -> loose PD wanted torque (w 4.5, zeta 0.65) -> `boot_steer` turns it into a leg swing + foot couple; thrust is applied only at the two soles (`boot_forces`, `apply_force_at_point`). Muscle PD is off while thrusting and fades out as the wingsuit opens.
- `player.rs`: new `wing_arch`, `leg_steer`. `body.rs`: rocket ankles follow `leg_steer` (what you see is the thrust line); skydive spine/arms/legs/head follow `wing_arch`/`wing_turn` instead of raw keys.
- Fixes "S rotates weird at speed": the old rocket mode thrust along `-cam_fwd` (body flipped 180 deg) and the skydive used target-velocity forces with a stiff slerp.
- Camera: keys no longer turn the view (only the mouse does; keys tilt the rocket aim by up to 0.6 rad pitch / 0.4 rad yaw while held). Third-person rig is rigid to the body: removed the `vel * 0.03` pivot lead (9 m ahead at 300 m/s, so the camera sat in front of the character) and the lateral spring; only vertical easing remains, on the ground.
- Wingsuit agility (`flyer.rs`): A/D are now ailerons (twist = key, roll rate 1.6 -> 3.6 rad/s: a full barrel roll in < 2 s, `wingsuit_holding_d_barrel_rolls`). Trim range 0.03..0.50 -> -0.30..0.65 rad: W pushes over into a ~52 deg dive in 1.9 s and holds it without bunting inverted (`wingsuit_w_pushes_over_into_a_steep_dive`); S/turns can pull much harder. View turn adds a `PULL` while heading error remains, released as the view comes round (stops the spiral): 90 deg turn 3.8 s -> 2.9 s. Near-vertical dives (exit, after a roll) are flown out before chasing the view (flat heading is noise straight down). Limb smoothing 5 -> 10 /s in `player.rs`. Check on the Mac: rolls not too twitchy, W dive feels controllable, exit from a standstill still pulls into a glide.
- Wingsuit fast-dive corkscrew fixed (`flyer.rs`): diving with the view off to the side slowly rolled the suit over. Two causes: bank was measured against world up, which is meaningless when the path is near vertical (new `path_bank`: bank about the velocity vector, reference = world up off the path, blended toward the view as the dive steepens); and W's negative trim flips lift to the belly, so banking steered *away* from the view. A lift floor now keeps arch at or above zero-lift while there's heading to make up. Test `wingsuit_fast_dive_looking_sideways_does_not_corkscrew` (worst roll 2.03 < 2.5 rad). Check on the Mac: W dive + look sideways turns toward the view instead of flipping.
- Sound plan: `docs/plans/sound.md`, every sound source in the game (body, air, rockets, vehicles, biome/water/weather/city beds, space, UI) tied to the data that drives it, plus a build order.
- Suit off (F) in freefall = a plain skydiver, no glide: `flyer::freefall_torque` (limbs in the airflow, authority grows with airspeed). W = forward somersault (4.5 rad/s), A/D = barrel roll (5.5 rad/s), S = spread eagle (+40% drag, firmer box), no keys = arches back to belly-down facing the view. `Player::fall_tuck`/`fall_spread` drive the ball-up / spread-eagle pose in `body.rs`. Skydive mode only (short hops in `Fall` keep the upright tilt).
- Check on the Mac: feel (too loose/stiff?), stability at 300+ m/s with Shift, rocket hover holds altitude, leg swing reads clearly, wingsuit doesn't stall in tight turns.

## Roads as geometry on graded terrain (B5-B8, partly verified)

  **Follow-up (hills):** decks are now level across (bed height from the centerline only, no lateral draping); `grade` snaps a flat shelf of `hw + cell` around graded roads so every terrain triangle under the road is at bed height; the outer curtain slopes down 1:1.5 to the actual terrain (up to 6 m) as an embankment/cliff. `TerrainGrid::upsampled(2)` runs on z14 tiles that have graded roads (192 -> 384 segments, ~4.6 m cells, exact same surface) so the bed can be cut narrower than a source cell; the bench is `hw + 1.45 * cell`. Unit test checks the deck against `grid.sample`. Measured on Aix before upsampling: terrain 0.9 m outside the deck edge was within +-2 cm of the centerline in 80% of samples, so on gentle ground the grading is nearly invisible by design. Cost to check with `--perf`: collider tris 4x on those tiles (~295k). Still to check on the Mac: embankments look tall on steep slopes (shelf is wide); satellite colour is still the original imagery (road bed and shoulder are not painted into the sat layer); paths/tracks (`Conform`) still drape and can tilt.
- `roads.rs` is now `plan` -> `grade` -> `generate`. `plan` decides bed heights per line (paved car roads: ground heights smoothed along the line; bridges: straight deck; paths/tracks/rail/service: follow ground). `grade(&mut grid, &plan)` runs in `tilegen.rs` right after `water::carve_terrain`: grid vertices near the centerline take the bed height exactly, easing back to natural over a cut/fill shoulder (3-14 m). The terrain collider is built from that grid, so paved roads now collide, and buildings/trees see the graded ground.
- `generate` sweeps a cross-section profile (`sweep`): urban = sidewalk + vertical curb face + gutter + cambered asphalt; rural = gravel shoulder + verge; with skirts so no cracks. Paths/tracks/rail use ground-hugging profiles (ruts, ballast) riding +9 cm above the terrain (they are NOT carved in).
- Markings are cut at exact dash boundaries (fixes missing short dashes). Guardrails: W-beam closed profile + posts every 3.8 m; bridges get deck rails. Street lights denser (28 m) and stand on the sidewalk.
- Tests: ribbon winding agrees with vertex normals, grading puts the bed under the centerline. Lookdev boards call `plan` without `grade` (flat grid).
- Check on the Mac: curbs/sidewalks at street level in a city (the city shot came out black here, unfocused window); the rural cut-bank edge looks like a low retaining wall (skirt 0.7 m vs shoulder grade); junction overlap, crosswalks, mile markers, roundabouts and a dirt-path ground-texture splat are not done yet.

---

## Smoother near terrain (unverified in-game)
- `ed_data::elevation` `HeightTile::sample_cubic` (Catmull-Rom): `TerrainGrid::from_heights` now samples bicubic instead of bilinear, so summits/ridges lose the pyramid creases. Passes through texels, so shared tile edges still agree.
- `tilegen.rs` grid segments: z14 64 -> 128 (~13 m, matches the z13 elevation texels), z13 64 -> 96. Before, the z14 mesh was already 192/side but every vertex lay on the 64-grid's triangles, so peaks were true 26 m facets.
- Side effects to check: z14 render mesh is now 128/side (was 192; per-vertex splat resolution ~13 m instead of ~9 m), z13 mesh 192/side (was 128), collider tris 8k -> 33k per z14 tile (`terrain::build_collider`). Check `--perf` and stalls when walking.
- Update: `tilegen::elevation_zoom` cap 13 -> 14 (Mapterhorn 512px WebP => ~3.3 m texels at z14, lidar where available; AWS z14 as fallback). Segments now z14 192 (render sub=1, so same 74k mesh tris as before and full splat res), z13 128. Collider at z14 is 74k tris/tile: if that hitches, build it only within ~1 km of the camera or decimate.

## 2026-11-04: leaf-card handoff stretching
- `foliage.wesl` / `foliage_prepass.wesl`: the card cull at ~590 m now decides per 4 m cell (cell centre distance, hashed band) and collapses a culled quad to its cell centre, instead of per vertex to the chunk origin. Per-vertex culling left half-collapsed triangles that stretched into slivers.
- Needs Mac check: no stretching where impostors meet 3D trees.
- Impostor grey cast ROOT CAUSE: the per-tree satellite tint (`in.tint`, `foliage.rs` `sat_linear`) replaced up to 90% of the atlas colour from 600 m. In shaded/alpine terrain that sample is a desaturated blue-grey, so whole forests went grey. A/B with the tint off gave green impostors (atlas albedo itself is green, checked with `dump_atlas`). Now `impostor.wesl` blends the tint at most 35%, only from 1.5 km to 6 km. Also kept: `shade` clamp 0.85-1.15 and the lit normal leaning to `up` with distance (flicker). Zermatt valley, lat 46.03 lon 7.74 alt 1900 heading 20 pitch -45 hour 14: forests read green; a few cool blue-grey flecks remain in shadowed spots (likely cool sky ambient). Needs a Mac look.

## Terrain ground: hex-tiled sampling, calmer bump (unverified near-ground)
- `terrain.wesl` `sample_planar`: replaced the 2-tap rotated blend with 3-tap hex stochastic tiling (per-vertex random offset + rotation, sharpened barycentric weights). Directional layers (5, 6, 9, 10) only shift. Normal rotation sign fixed (`-ang`). One plain tap beyond 1200 m (`hexw`).
- `ground_tex.rs`: grass/dry grass bump 0.7 -> 0.3, forest floor/moss 1.0 -> 0.6 (bump is applied in the baked normal AND in the shader); world size grass 6 m, other 3 m layers 4 m.
- Earlier this session: footprint-banded far octaves, noise-ragged splat edges, snow luminance lift + lower snow bump, `SAT_CLASSES` 512px pool 40 -> 72.
- Check on the Mac: walk on grass at ground level (repeat/bumpiness), `--perf` (3 taps per layer within 1.2 km), snow whiteness, far terrain detail.

## 2026-11-04: far shadows past the cascades (B3 stage 2, on by default; `EW_FAR_SHADOWS=0` off, `=debug` magenta tint)
- Far tree impostors receive the horizon far shadow too (per-tree lookup at the tree's tile uv; `EW_FAR_SHADOWS=debug` tints them cyan). Check on the Mac: forests on slopes in ridge shadow go dark beyond the cascades, no popping at the cascade edge.

Horizon maps (work from orbit), canopy darkening and a short surface-height march, all baked per tile on the worker and stored in a pooled texture array (`ed_world::farshadow`, `ed_game::far_shadows`, block in `terrain.wesl`). Details and limits are in `docs/plans/shadows.md` under "Stage 2 landed".
- Verified: tests pass. Headless Everest from 65 km at 07:00 shows correct west-pointing ridge shadows.
- Mac check:
  - GPU cost on/off with `--perf` over mountains.
  - Canopy and march in clear weather at 5-15 km AGL.
  - Tile seams in the horizon term.
  - Pool exhaustion warnings ("far-shadow pool full").

## 2026-11-04: shadow cascades, stage 1 (B3) + horizon-map prototype

Plan: `docs/plans/shadows.md` (layered: cascades near, cached mid map, terrain horizon maps far).
- `ed_runtime::shadows` (new, 5 tests): `CascadePlan::for_agl`, 4 cascades. Walking: bounds 15/83/454/2500 m, texels 1.8 cm/9.5 cm/52 cm/2.9 m (was 2 cascades, 4.6 cm then 80.5 cm with a 17x jump at 34 m and a hard cut at 700 m). Flying: reach grows to 12 km (11.6 km at 1500 m AGL). AGL is quantized to quarter octaves so plans don't breathe.
- `ed_game::shadows` (new plugin): ray-casts down every 8 frames and swaps the Sun's `CascadeShadowConfig` only when the plan changes. `sky.rs` spawns the Sun with the plan for the start AGL. `EW_SHADOWS=legacy` restores the old config.
- `vendor/bevy_pbr/src/render/shadows.wesl`: the last cascade fades to lit over 80-100% of its far bound, so the hard cut is gone. Listed in CLAUDE.md's patch list.
- `ed_world::horizon` (new, 6 tests, not wired): per-tile horizon-map bake (33x33 points x 16 azimuths, 30 km reach, curvature drop, sqrt-u8). Measured 22.8 ms and 17 KB per tile. Wiring to `terrain.wesl` behind `EW_FAR_SHADOWS=1` is the next step (needs a coarse elevation parent in `tilegen::prefetch`).
- Verified headless (`--fly --weather clear --hour 14`): Zermatt 40 m, building shadows at 60-150 m keep their outlines (were blobs); Central Park 25 m, crisp; Zermatt 1500 m, terrain shadows in the valley (legacy: none). Not fixed: towers past 750 m cast nothing (`SHADOW_EXPLODE_M`, `streaming.rs`).
- Cost unknown within noise: three 22 s Zermatt runs gave medians of new 27-32 ms vs legacy 24-30 ms vs no shadows 26.7 ms. Estimate 0-8 ms per frame.
- Check on the Mac (see the plan's checklist): repeated A/B of default vs `EW_SHADOWS=legacy` vs `--no-shadows` (window focused, ~25 s each); Central Park at 2 m, no seams at ~15/80/450 m and a smooth fade at 2-2.5 km; rocket to 3 km, no flash when plans switch; walk > 8 km, watch for a rebase shimmer.

## 2026-11-04: tree card/impostor handoff flicker

Cause: impostors were cut per tree at ~650 m, but leaf-card chunks exist per ~300 m chunk out to 800/860 m, so trees at 650-1000 m were drawn twice (z-fighting) and cards hard-popped per chunk.
- `tilegen.rs`: impostor near cut = `CARD_RANGE_M - 210` (590 m, was 650).
- `foliage.wesl` + `foliage_prepass.wesl`: `card_culled` collapses card vertices past 570-610 m (per-vertex hashed band, perspective views only so shadows are unchanged). Impostors fade in over the same band, so the tiers are complementary.
- Chunk spawn range (800/860 m) still covers every tree within 610 m (chunk half-diagonal ~212 m).
- Gotcha: the `view` import is `bevy_pbr::render::mesh_view_bindings::view` (copy `impostor.wesl`). A wrong path only logs a `shader_cache` WARN "unresolved import" and the material silently draws nothing (full trees vanished). Grep run logs for `unresolved`, not just `error`.
- Verify on the Mac: look across a forest at ~500-700 m; no shimmer line, no gap, no double canopy. Headless run: no shader errors.

## 2026-11-04: impostor flicker (B2) and shiny ground (B4)

- `shaders/impostor.wesl`: no more snapping between the 8x8 atlas views. The fragment blends the 4 views around the live direction (bilinear in the octahedral grid), each reprojected with its own billboard basis, alpha-weighted; the alpha test runs on the blended alpha. The vertex shader passes the live basis (`br`, `bu`) and view dir (`vl`) as flat varyings. Thinning ramp widened (0.2 -> 0.4 of share); sub-pixel fade now ramps over 0.8x-1.3x `MIN_PX`, and `MIN_PX` is 2.5 (was 1.5, a ramp of 0.75-1.5 px), so quads under ~2 px (which the rasteriser drops at random) are gone.
- `shaders/terrain.wesl`: dry land gets a roughness floor of 0.8, rising to 0.97 from 25 to 500 m; reflectance falls up to 70% with distance (60-1200 m) and specular occlusion drops to 0.3 far out. Water keeps its own gloss; wet ground stays glossy near.
- Headless check (Manhattan, 1500 m, 90 frames): shaders compile, forest and water look right. Flicker cannot be judged from a still.
- Check on the Mac: fly over forest at 1-5 km and turn the camera (no ticking between views, a little ghosting at most); stay still at altitude (no shimmer); ground should look matte at all ranges, sun glint only on water. Cost: 8 texture fetches per impostor fragment (was 2); watch fps over dense forest.
- Not touched: shadows (B3), tracked in `BUGS.md` with a separate design prompt.

---

## 2026-11-04: building detail gets a mid level (triangles per pixel)

- `bparts::Out::lite` + `buildings::build_detail_lite` / `DetailSet::build_chunk(c, lite)`: same model minus sub-pixel parts (mullions/transoms, keystones, hood cornices, grilles, louvres, door/garage panels, balusters/planks -> solid panels, flower heads -> one block, low-poly bushes, no bevels).
- `streaming.rs`: full kit inside `NEAR_IN_M` 70 m, mid level beyond `NEAR_OUT_M` 100 m (hysteresis); level swaps despawn the old chunk in the same flush the new one spawns (no holes) and queue after first loads.
- Measured (headless still Zermatt, 2144x1206): detail 18.1k -> 4.9k tris/building in mid; visible building/other tris 3.15M -> 1.25M; fps ~33 -> ~42.
- `EW_TILE_STATS=1` prints `DETAILSTAT lite=..` per chunk build.
- Check on the Mac: pop when chunks swap at 70-100 m; facades at 100-230 m still read.
- Next: far z14 terrain without the x3 splat subdivision (73.7k -> 8.2k tris/tile, geometry identical; 1.94M visible now), shader pipeline pre-warm, explode hitches.

## 2026-11-04: species as data + genomes, tree and rock collision

Plan and next steps: `docs/plans/procedural-flora.md` (step 1 of 6 landed).

| Item | Where |
|---|---|
| `SpeciesDef` table (leaf palettes, far-card budgets, trunk colliders, genome ranges) and `Genome` (+ `sample`, `apply`) | `ed_world/src/trees/species.rs` |
| `build_genome`; `build` = default genome, tested identical | `ed_world/src/trees/mod.rs` |
| Trunk cylinders, one static compound per tree chunk, full-detail tier only | `ed_game/src/foliage.rs` `tree_colliders` |
| Rock trimesh colliders within 90 m, lazy (3 builds/frame) | `ed_game/src/near_chunks.rs` |

Needs checking on the Mac: walk into an oak, a baobab and a rock (should stop, not clip); no frame hitch entering a rock field or dense forest (compound build happens on the chunk swap); walking along a slope does not snag on trunk bases. The genome is not used per instance yet, so the look is unchanged.

---

## 2026-11-04: headless perf harness, shadow proxies, half-res clouds (measured on M3 Pro)

All numbers come from `tools/perf_headless.sh OUT [scene...]` (`--headless`: no window, no focus, no sound; 2144x1206, `--weather fair`, scenes 22-25 s). The stats skip the load phase.

| Item | Where | Notes |
|---|---|---|
| Headless mode | `headless.rs`, `config.rs` (`--headless`, `--cruise`, `--spin`, `--quit-after`) | Renders to an offscreen image. `EW_GPU_WAIT=1` logs the GPU wait per frame. `--screenshot` still quits after the capture, so don't combine it with perf runs. |
| GPU bisect | `perf.rs` `EW_BISECT=hide:cat,noshadow:cat` | Categories: bld, std, terrain, foliage, scatter, cloudsky, cloudshadow, all. Metal has no per-pass timers; this is how cost gets found. |
| Shadow proxies | `streaming.rs` (terrain, buildings), `foliage.rs` (full trees) | Detailed meshes render on `CAMERA_ONLY_LAYER` (6). The sun's shadow pass draws a cheap stand-in on `CLOUD_SHADOW_LAYER` (7). Full trees cast through their card. Cruise went from 17.4 to 34.9 fps (min 7.2 -> 25.2). |
| Half-res clouds | `cloud_half.rs`, `clouds.wesl` (`CLOUD_HALF`), `clouds_up.wesl` | The cloud raymarch runs at half resolution after the sky pass. Each 2x2 block marches against its near or far depth in a checkerboard pattern. The result is composited with a depth-aware 4-tap upsample. The material mesh is hidden; it still feeds the uniform. Still scene: 27.0 -> 24.7 ms. `EW_CLOUD_FULL=1` restores the old full-res path. |
| Vendor patch | `vendor/bevy_pbr/src/atmosphere/mod.rs` | Adds `RenderSkySet` (the sky pass as a system set) so other passes can order after it. |

**Check on Mac (windowed):** cloud edges against mountain silhouettes, with no halo or blockiness. Also check that tree trunk shadows look right on foot, since the proxy card casts in place of the full tree.

**Open (M0 exit not met yet):** still 40 fps; startup/explode spikes of 100-480 ms in cruise and origin. Next: pipeline pre-warm, then smoothing the building-detail explode.

---

## 2026-11-04: real-data ground cover, trees and biomes

Deserts no longer get lawns and dense forests are no longer sparse. Everything is deterministic and works offline (OSM-only fallback); nothing is run on Metal yet.

| Item | Where | Notes |
|---|---|---|
| Climate biomes | `ed_data::climate` (+ `data/climate10.bin`, `tools/bake_climate.py`), `biome.rs` `from_climate` | WorldClim 2.1 normals, lapse-rate corrected. New `Biome::Steppe`. Old lat/lon boxes only where the grid has no data. |
| Land cover | `ed_data::{worldcover, cog}`, `http::get_range`, `SurfaceMap::with_worldcover`, `tilegen.rs` (z >= 8) | WorldCover COG range reads fill only `Natural` cells; OSM polygons win. New `Cover::{Shrub, Bare, Moss}`. |
| Greenness gate | `SurfaceMap::splat`, `greenness` | Where the satellite is not green, grass/natural layers give way to dirt/gravel/sand. |
| Canopy-driven trees | `SurfaceMap::tree_density`, `foliage::scatter` | WorldCover tree/shrub fractions x per-biome max density. Up to 3 plants per 8 m cell (was clamped to <1). Per-tile caps raised 9k/6k/5k/4k -> 16k/10k/7k/5k. |
| Geology | `ed_data::geology` (Macrostrat), `SurfaceMap::lith` | Scattered rocks take the bedrock colour (tiles z >= 10). |
| Per-tree variation | `foliage::vigor`, `scatter`, `trees::instance_rotation` | Size from age x biome/altitude vigour, lean (a few degrees), per-crown tint jitter. Still 3 mesh variants per species. |

**Not done:** canopy comes from WorldCover tree fraction, not ETH/Hansen height (ETH unreachable; Hansen tiles are 53 MB stripped). Sand/scree ground textures are not yet tinted by lithology (shader work). More than 3 mesh variants would grow the impostor atlas.

**Check on Mac** (short runs, window focused): `--place "Zermatt"`, a Sahara drop (`--lat 23.4 --lon 25.6 --alt 800`), Manaus (`--lat -3.1 --lon -60 --alt 600`), Aix, Central Park. Expect: bare sand/gravel and almost no trees in the Sahara; thick canopy at Manaus without holes; no regression in cities. Watch tile build time (first run fetches WorldCover blocks; later runs read the cache) and tree counts per tile with `EW_TILE_STATS=1`.

---

## 2026-11-04: pause-menu map ignores drag/click (fix)

Bevy's `ui_focus_system` stops updating `RelativeCursorPosition` on hidden nodes, so the "Drop in here" popup, hidden while the cursor was on its button, kept `cursor_over = true`. `globe::control` read that as "cursor over popup" and dropped every press, drag and scroll on the map from then on (the first drop from the main menu worked; every pause map after it didn't). Fix: popups (and the time slider track) only count when `InheritedVisibility` is visible. **Check on Mac:** drop in, Esc, then drag, click to pick and scroll the map; drop again from the popup, then do it once more.

## 2026-11-04: perf milestones M0 + M1 (on `main`)

Built and unit-tested in a Linux cloud container with a software GPU. A smoke run renders Zermatt with no shader errors, but **nothing is measured on Metal yet**. The Mac pass below decides whether M0's exit criterion is met (45+ fps steady, no frame > 50 ms after load).

### What changed

**M0, in Bevy**
| Item | Where | Notes |
|---|---|---|
| 2 shadow cascades | `sky.rs` `setup_sky` | 40 m near, 700 m far (was 3). |
| Env map only on change | `sky.rs` `refresh_env_map`, patched `vendor/bevy_pbr` (`EnvironmentMapRefresh`) | The atmosphere probe and env-map filter passes run only when the sun moves > 0.3°, altitude changes > 3% (min 50 m), cloud factor changes, or the probe is re-added. 3 frames per refresh. Was every frame (~2 ms). |
| Shared terrain material | `ground_material.rs` `SatPool`, `shaders/terrain.wesl` `sample_sat` | Satellite colour lives in 3 texture arrays (512 px x 40, 256 x 96, 128 x 192; 76 MB). The layer rides on the terrain entity's `MeshTag`, so all terrain batches under one material. Also fixes ground textures jumping after > 8 km of travel (per-tile material copies missed the floating-origin UV update). |
| Budgeted, staged tile apply | `streaming.rs` `apply_tiles`, `ed_runtime::budget` | Per-frame cost budget (`EW_APPLY_BUDGET`, default 250 units) plus a 4 ms cap. A tile stays hidden until complete, so the coarse tile keeps covering. |
| Tree impostors | `ed_world::trees::impostor`, `impostors.rs`, `shaders/impostor.wesl` | User approved impostors for trees. See below. |

**M1: `ed_runtime` (engine-free, 19 tests) and streaming on it**
| Module | What |
|---|---|
| `jobs` | Keyed thread pools: priority, re-rank while queued, cancel, one entry per key, panics reported. |
| `budget` | `FrameBudget`: cost units + wall clock; the first item always runs. |
| `residency` | `Lru` with a cap. |
| `view` | Cone test using the tile's real height range. Fixes tiles vanishing above ~4 km ground (Altiplano, Tibet) and when looking up at peaks. |
| `scheduler` | Build priority (visible first, prefetch along travel above ~50 m/s) and hole-free retire, linear time. |

Streaming now runs as: IO pool (8 threads, downloads into the disk cache) -> CPU pool (cores - 3, max 8, `tilegen::build` + mesh prep) -> budgeted apply -> retire. Stale queued tiles are cancelled at each select (4 Hz). Out-of-view tiles stay loaded while still in the LOD set (`EW_RESIDENT_TILES`, default 192), so turning around doesn't rebuild the street behind you.

**Tree impostors**
- Atlas: a CPU rasterizer renders all 69 species variants (23 kinds x 3) from 8x8 hemi-octahedral views. 32 px frames, 4x supersampled, albedo + tree-local normal, coverage-preserving mips. About 5 s in release, on a background thread at startup; impostors draw nothing until it lands.
- One quad per tree, one impostor mesh per tile (`impostor::build_mesh`, built on the tile worker).
- Shader: billboards toward the camera, picks the nearest view frame, thins with distance (share shown = (1000 m / d)^2), grows survivors up to 6x so canopy cover holds, fades trees under ~1.5 px. Past 600 m it blends toward the satellite colour under the tree (`TreeInstance::tint`), so far forest matches the photo instead of speckling it. Same vertex code in main pass and prepass. No shadows.
- Trees now exist on z10+ tiles (were z13+). Caps: z14 9,000; z12-13 6,000; z11 5,000; z10 4,000 (`tilegen.rs`).
- LOD chain: impostor -> leaf cards per chunk (z14, within `CARD_RANGE_M` = 800 m) -> full trees (within 230 m). The old thinned card bands are gone.
- Range: a 10 m tree is ~1 px at ~11 km (1280 px, 60° FOV). With growth, forests read as canopy texture to roughly 20-30 km; past that the satellite colour carries them, which is what forests look like from orbit.
- Tuning: `FULL_M`, `GROW_MAX`, `MIN_PX` at the top of `impostors.rs`.

**Tiles stop being entities (M1, Bevy form).** A tile is now ~10 entities whatever its content:
- terrain, water, roads;
- library placements merged into one mesh + one compound collider (`library::merge_placements`; the `EW_MESHLET=1` path is unchanged);
- building LOD merged per material + one trimesh collider (`streaming::merge_lod`). A tile explodes into per-chunk LOD entities only while building detail loads near the player, and merges back after 8 s with nothing near (`streaming::Explode`);
- one impostor entity for all its trees;
- a `TreeChunks` holder: leaf cards and full trees are entities only near the camera (`foliage::stream_tree_chunks`);
- a `NearMeshes` holder: rock chunks are entities only within 500 m (`near_chunks.rs`).

The fully GPU-resident form (no per-tile entities at all) comes with M2's own renderer.

**Fixes found along the way**
- `terrain.wesl` used `class` as a variable name (reserved in WGSL), so the terrain shader failed at runtime. Caught by the software-GPU smoke run.

### Measured in the cloud box (software GPU: counts only, fps meaningless)
Same camera, old build vs new:

| Scene | Entities | Visible meshes | Load to idle |
|---|---|---|---|
| Zermatt village, 120 m AGL | 16,503 -> 2,166 | 4,233 -> 890 | 178 s -> 73 s |
| Zermatt, 4 km | 12,892 -> 1,334 | 2,188 -> 117 | 146 s -> 50 s |
| Black Forest, 3 km | 7,397 -> 1,247 | 96 -> 108 | 82 s -> 26 s |

Screenshots match the old build, plus more distant trees. They were taken at 640x360 with a software renderer, so they look soft; that's the capture setup, not the game.

### Not done, on purpose
- **Pipeline pre-warm** (M0). Likely cause of the 3-6 s settings stall: the AO and TAA toggles (`menu.rs` `apply_graphics`) add/remove camera components, which changes every material's pipeline key. Pre-warming needs a second hidden camera with the other key (21 systems assume one `Camera3d`) or a `bevy_pbr` patch that keeps SSAO bound and skips its dispatch. Confirm the cause first (check 4).
- **Clouds ½ res** (M0). Measured 33.3 vs 33.5 fps with clouds off: no gain while vertex bound. Revisit after M2.

### Mac test pass (about 20 minutes)
Use Zermatt plus the plan's Ohio suburb at 67% render scale, and compare against `c42ed5e` (before this work) at the same spot.

```bash
EW_PERF=1 EW_PERF_OUT=perf_m0.csv cargo run --release -- --place "Zermatt"
```

1. **Steady fps**: stand still 30 s after load; `fps` and `p99_ms` in the CSV. Target 45+.
2. **Streaming hitches**: walk/fly 60 s; `max_ms` under 50 after load. Spikes: lower `EW_APPLY_BUDGET` (try 120). Slow loading: raise it (try 500). `io_jobs`, `build_jobs`, `applying` show where tiles wait. `entities` should stay roughly flat.
3. **Turn around** at street level: tiles behind you don't reload (F3: `tiles N loaded` flat, `pending` ~0).
4. **Settings stall**: toggle only AO, then only Bloom, then only Shadows. If only AO (and TAA) stall, the pipeline-key theory holds.
5. **Forests**: fly over Zermatt and the Black Forest (`--lat 48.0 --lon 8.2`) at 300 m, 3 km, 10 km AGL. Trees fade into the satellite colour without popping or bald hills. No ring at 650-950 m where trees vanish or double in size (card/impostor seam). Impostors appear a few seconds after start (atlas bake).
6. **Buildings**: walk into town; detail swaps in, no flicker when a tile explodes or merges back.
7. **Physics**: drive into a street light and a building on a freshly loaded tile (merged colliders).
8. **Visuals from M0**: satellite colour on distant terrain and water as before (F3 shows `satellite layers free a / b / c`; if one hits 0, raise it in `SAT_CLASSES`). Ambient light follows time scrubbing (`[` `]`), no black ambient after spawn or the pause menu. Near shadows still crisp with 2 cascades.
9. **High ground**: La Paz (`--lat -16.5 --lon -68.15`) and the Altiplano above 4 km; look level and up at peaks. No missing tiles.
10. **Floating origin**: fly > 8 km in one direction; ground textures don't jump.

If something looks wrong, the pieces are separate commits (`dfa506c`, `0598c84`, `fd3d90b`, `1b73019`, `4a27765`, `0425426`) and can be reverted one by one.

---

## 2026-11-03: tile culling and space targets

- `streaming.rs`: the desired tile set is filtered against the camera's view cone before builds start. Superseded on Nov 4 by `ed_runtime::view` (height-aware) and the residency cap.
- `sky.rs`, `hud.rs`: celestial body meshes and their HUD rings/labels appear only above 100 km altitude.
