# Changes

## 2026-11-05: native shadows to 20 km (cached), screen-space contact shadows
- `world.rs`: cascades 4 (to 4 km) and 5 (to 20 km) are now camera-centred spheres, so turning the camera no longer changes their matrices; the existing per-cascade hash (matrix + caster list, texel-snapped positions) then skips the re-render until you move a texel (`shadow_skipped`). Cascade 5 now casts buildings too (casters smaller than 1.5 texels, ~29 m at 20 km, are culled; terrain always casts).
- `post.metal` `post_contact` + `post.rs`: port of Bevy's `ContactShadows` (12 steps, 0.25 m thickness, 1.5 m reach, scaled with distance). Marches toward the sun through the linear depth buffer, darkens HDR by 0.45 x N.L-gated, in place before SSR. `EW_CONTACT=0` disables it (same switch as Bevy).
- Known: NYC window frames render detached from the facades (visible at street level in `ct1.png`); unrelated to shadows, still to fix.

## 2026-11-05: volumetric clouds in the native Metal renderer, stable at every altitude
- Port of `clouds.wesl` to `ed_metal/shaders/clouds.metal` + `ed_metal/src/clouds.rs`. Passes between the main pass and post: half-res compute march (checkerboard of the 2x2 block's nearest/farthest depth, clipped against scene depth), temporal resolve (reproject by cloud-weighted hit distance, depth validity, 3x3 variance clip), depth-aware bilateral composite premultiplied onto HDR (so AO/SSR/bloom/MetalFX see clouds). Noise bytes come from `clouds::CLOUD_NOISE_CPU`, the flat uniform from `clouds::CLOUD_FLAT` (both set by the existing Bevy cloud systems, which still run the climate/wind/storm logic).
- Altitude fixes (marked `STABLE:` in the shader): weather coverage sampled on a fixed sea-level plane (it slid with camera altitude before); reach grows with the horizon distance above the layer and fades out instead of a hard 300 km ring; step size coarsens to cover the whole span in the step budget; mip LOD on the noise from step length (no distance sparkle); sun elevation at each sample's own vertical; haze uses the native `sky_rad` so cloud and fog agree. Addresses B11.
- Default weather is now `clear` (5% few small cumulus) instead of the live report: `--weather live` or `EW_WEATHER=live` restores it; lookdev still uses `fair`. Native clouds are on by default on macOS (`EW_NATIVE_NOCLOUDS=1` / `--no-clouds` to disable).
- Cost: not yet measured on the Mac (headless shots only). Estimated 1.5-3 ms at 1440p HIGH. Off switch `EW_NATIVE_NOCLOUDS=1`, lux->HDR gain `EW_NATIVE_CLOUD_GAIN` (default 6.4/130000), quality via `--cloud-quality`, `--no-clouds`.
- Checked headless (43.53, 5.45, `--weather fair`): ground, 800-9000 m sweep and straight-down at 3.5/8/20 km show a continuous cloud field with no pops at the layer boundaries.
- Check on the Mac: fly a climb through the layer at speed (B11), look for shimmer at cloud edges, MetalFX on/off, sunset colour vs the old Bevy look, GPU ms (`times`). Known gaps: no cloud shadows on terrain in the native path, clouds are not in SSR reflections of water below, clouds still look softer than Bevy's (mip LOD / detail fade to tune).

## 2026-11-05: native shadows reach buildings and trees out to 4 km
- `ed_metal/src/world.rs` + `world.metal`: 6 cascades. 0-3 unchanged (to 700 m), new cascade 4 (700 m - 4 km) casts everything (buildings, trees, terrain; small casters culled by texel size), cascade 5 is the old terrain-only far cascade (to 20 km). Previously building/tree shadows ended at 700 m.
- Check on the Mac: NYC low sun from the west at 1-3 km; shadow map cost (one more 2048 layer).

## 2026-11-05: boot rocket flames reworked
- `boot_plume.rs`: three nested triangle flame layers (wide at the nozzle, taper to a tip), 2.6-8.6 m long, vibrating (95 rad/s shear) and bending back along -velocity (`f^2` displacement, up to 0.55 of length at 40 m/s).
- No HDR gain: colours stay <= 1 so bloom no longer whites out the scene under the flame. Smoke particles live 1.4-2.8 s and grow 3.5-5.5x for a trail.
- `body.rs`: the emissive inner core cone is hidden whenever the plume runs (it was the bloom-white blob at each nozzle).

## 2026-11-05: probe GI in the native renderer, sun-aware; AA mode selector
- Menu "Upscaler / AA" is now one mutually exclusive choice (MetalFX upscaler / Anti-aliasing / Off, `menu::AaMode`); the upscaler does its own AA so FXAA is skipped under it. FSR 4 (community Metal port, macOS 26+) is planned as a fourth entry.
- Probe GI: the Bevy bake (`ed_game/src/probe_gi.rs`, `ed_world::probes`) is reused; `native::send_probes` uploads the volume to a 3D RGBA16F texture (`WorldRenderer::upload_probe_volume`, texture slot 8) whenever it re-bakes. `probe_gi()` in `world.metal` ports `irradiance_volume.wesl` (terrain-following layers, ambient-cube faces) and `shade_ex` blends `amb*skyVis + up*bounce`. `EW_PROBE_GI=0` turns it off.
- Sun-aware: `ProbeParams.sun`; each bounce ray's hit is tested toward the sun, so shaded ground only returns sky fill (0.25 of albedo) and sunlit ground full albedo; bounce is scaled by `1+4*sun elevation`. The volume re-bakes when the sun moves ~4 degrees.
- Bug found: `manage_cameras` deactivates the Bevy 3D camera in native mode and probe baking required an active camera, so probes never ran natively; `camera_pos` now falls back to any 3D camera.

## 2026-11-05: native far-tree impostors
- Impostor tiles now draw in the Metal renderer. `impostor_vs/fs` in `world.metal` port `impostor.wesl` (hemi-octahedral 8x8 atlas, 4-view blend, thinning by distance, growth to cover thinned neighbours, satellite-tint blend + smoothed normals far away), lit through `shade_ex` (rough 0.95) so they get the same sun, cascaded shadows, haze, AO/bloom/MetalFX as everything else.
- Plumbing: `WorldRenderer::upload_extra` (params+axis+tile-uv stream, 10 floats/vertex), `upload_impostor_atlas` (2D-array albedo sRGB + normal, all mips per layer), `DrawItem::impostor`; `native::send_atlas` hands over the baked atlas (impostor mesh/atlas images now use `native::mesh_usage()` so the CPU data survives). Impostor meshes skip cluster simplification and their cluster bounds are padded by the grown radius. Viewport height/tan(fov/2) rides in `FrameU.fog.z`.
- Checked headless in the Olympic Peninsula forest (47.55, -123.65, alt 500): crowns cover the ridges to the horizon, shaded slope reads dark.
- Not ported: the far-shadow horizon term for trees (`far_tree_shadow`), so distant crowns do not darken in a far ridge's shadow beyond the 5 cascades; no specular-occlusion tweak.

## 2026-11-05: menu graphics options drive the native renderer
- `native_frame` reads `GraphicsSettings` every frame: shadows, ambient occlusion, bloom, anti-aliasing (FXAA-lite at native scale), and render % (-> MetalFX internal scale; 100% = native). New `WorldRenderer::set_options`. `EW_NATIVE_SCALE` still overrides the render %, and `EW_GFX=...` works as before.
- Verified headless: shadows off -> 0 shadow draws; res=100 turns MetalFX off. Resolve stays ~7.8 ms at native scale even with AA/AO/bloom off, so that cost is the full-res resolve itself (not FXAA); MetalFX at 0.67 brings it to ~0.8 ms.
- Still open: SSR / bounce toggles and an upscaler-mode choice (MetalFX vs FSR 4) are not in the menu yet.

## 2026-11-05: MetalFX temporal upscaler (native renderer)
- New `ed_metal::upscale::Upscaler` (`crates/ed_metal/src/upscale.rs`, `shaders/upscale.metal`). The world now renders at `EW_NATIVE_SCALE` (default 0.67) of the output size with a Halton(2,3) jittered projection, then `MTLFXTemporalScaler` brings it to full res before `resolve_fs`. `EW_NATIVE_SCALE>=0.99` disables it (native res + FXAA-lite).
- Inputs: HDR colour, Depth32F (reversed-Z), RG16F motion computed in a compute pass from depth + previous view-projection (camera motion only; moving objects/boots may ghost slightly, per-object velocity is future work), auto-exposure, reset flag on camera jumps > 300 m.
- `EW_NATIVE_NOAA=1` skips the FXAA-lite pass. With MetalFX on, FXAA is skipped automatically.
- Headless NYC check: scale 0.67 is visually equal to native with clean edges. Resolve time looked inflated at scale 1.0 (machine busy); re-measure on an idle Mac.
- Not yet tested: moving camera (jitter/motion sign), boots under MetalFX.
- Second option planned: AMD FSR 4 via the community Metal port (macOS 26+, unofficial); FSR3 source is DX12/UE5 only.
- Nanite-like options (Windows/Mac) research is still to be written up in `docs/plans/engine.md`.

## 2026-11-05: native Metal is now the default renderer; AA; research; what is left
- `native::enabled()` is on by default on macOS. `EW_NATIVE=0` falls back to the Bevy renderer. Headless NYC check (Central Park, 1600x900): looks right, 27 fps under load (world 19 ms; machine was busy, re-measure idle).
- `resolve_fs` now has an FXAA-lite pass on compressed HDR luma (blend along the edge tangent only where contrast is high). The resolve timer read 14 ms in that run: check on an idle Mac whether the AA or timer attribution is to blame.
- Native renderer ported so far: cascaded + far shadows, GGX PBR, sky/haze, GTAO, SSR, bounce, bloom, filmic grade, splat-layer terrain with normal mapping, alpha blend pass, boots/plume (`boot_plume.rs`).
- NOT ported yet (in priority order): (1) [DONE, see impostors entry] far tree impostors; (2) probe GI (`probe_gi.rs`: small ground probes, large air probes, ~5 km, coarser farther); (3) 20 km buildings (`ed_world/src/lod.rs` floor is 16 km) and far Manhattan shadows (building shadow range 700 m); (4) more contrast in the grade; (5) menu option wiring on the native path (render scale / FSR options only affect the Bevy path, `render_scale.rs` returns early when native owns the world).
- Upscaling plan: `ed_game::render_scale` FSR1 EASU stays Bevy-only. For native use **MetalFX** (Apple, macOS 13+, iOS 16+): the spatial scaler needs only a colour texture; the temporal scaler (DLSS2/FSR2 class) needs colour + depth + motion vectors + jitter, so add a jittered projection and a velocity target to `WorldRenderer` first. Also an option: AMD FSR 4 community Metal port (macOS 26+, unofficial).
- Prior art searched: no open-source Nanite-style renderer exists for Metal (examples are D3D12/Vulkan: Nyx, Vulcanite, jglrxavpok "Recreating Nanite", Bevy virtual geometry write-up by jms55). Metal 3 has mesh shaders, so the cluster-LOD + visibility-buffer plan in `docs/plans/engine.md` is ours to build; borrow the algorithms (METIS clustering, 64 vert/124 tri clusters, normal cones).


## 2026-11-05 Native Metal: splat-layer terrain (normal mapping)
- `terrain_vs`/`terrain_fs` in `ed_metal/shaders/world.metal` port Bevy's `terrain.wesl`: 12 splatted layers (albedo+height, normal+rough+AO arrays), hex stochastic tiling, triplanar on steep faces, height-based blend, per-layer size and bump (grass small normals, rock large), per-pixel snow/rock rules, strata, macro variation, satellite steering, water. Skipped beyond 5 km (satellite only there).
- `WorldRenderer::upload_ground` / `upload_splat`; `ed_game::ground_material::GROUND_CPU` hands the layer data over once; `native.rs` reads `ATTRIBUTE_SPLAT0/1/2`. Off switch: `EW_NATIVE_NOGROUND=1` (old noise-bump terrain).
- Matterhorn 1600x900 headless: world pass ~5-9 ms with layers (about equal to the old path once far pixels skip the loop). Needs a look at close ground level (grass vs rock scale) on the Mac.

## 2026-11-05: native SSR + screen-space bounce, AO cost cut
- `ed_metal/src/post.rs`: AO defaults to 2 slices x 5 steps (was 3x8): NYC 450 m 37 -> 46 fps. Added `post_ssr_trace` (half res) + `post_ssr_composite` (full res, HDR target now has `ShaderWrite`), plus a 3-level unthresholded downsample chain (1/8 res scene blur) used by `resolve_fs` as bounce light: `h += blur * (1 - ao) * 0.5` before the AO multiply.
- Off switches: `EW_NATIVE_NOSSR`, `EW_NATIVE_NOBOUNCE` (with `NOAO`, `NOBLOOM`).
- Cost: SSR ~0.4 ms, bounce ~0.1 ms at 1600x900 (NYC resolve 6.9 -> 7.3 ms). Later runs on a loaded Mac showed Matterhorn 53 and Mexico City 46 fps, but Matterhorn with all post off was 54 fps, so that is machine load, not the new passes. Re-measure on an idle machine.
- Look: bounce subtly warms street canyons; SSR adds faint reflections on water/glass. Both are subtle. Water raw roughness vs the 0.55 SSR threshold is unverified.

## 2026-11-05: native GTAO + bloom (post chain)
- `ed_metal/src/post.rs` `PostChain`: linear depth, half-res GTAO (bilateral blur + upsample), dual-filter bloom, from `shaders/post.metal`. Runs between the main pass and the resolve; resolve multiplies AO (0.85) and adds bloom (0.06). Radius grows with distance (3 m up to 30 m) so it reads from the air.
- Switches: `EW_NATIVE_NOAO`, `EW_NATIVE_NOBLOOM`, `EW_NATIVE_AOVIS` (show AO). NYC 450 m: 43 -> 37 fps (about 3 ms). Also fixed: a bright vertical streak under the sun in native views toward the sun (terrain fog sampled `sky_rad` inside the sun disc; fog now passes `disc_k = 0`). Matterhorn raking view 75 fps; Mexico City 3500 m 63 fps with AO+bloom. Terrain shader now lifts green in magenta-cast Sentinel texels (pink scrub patches) and rock is slightly less red. Open: halve cost (AO at quarter res or fewer steps), bloom is barely visible, SSR/bounce not yet integrated.

## 2026-11-05: far buildings, sharper satellite, roof/sun tuning (native)
- Satellite: `ed_data::satellite::MAX_ZOOM` 12 -> 14 (EOX s2cloudless has real ~10 m detail at z14; z14 terrain tiles were sampling a z12 tile = 16 px per tile). Sat texture 128 -> 256 px for z>=10 tiles; pool class 256 now 320 layers.
- Far buildings: z13 tiles now emit OSM building massing (`build_lod` only, no roads/props/detail); `lod::select` forces z>=13 within 16 km ground distance so buildings reach ~16 km. Mexico City hard cut-off line gone; 58 fps, 4.5 M tris.
- Native shader: cascade-4 normal bias 2.0 -> 0.8, sun 6.4, warmer low sun, `day` ramps 6x with sun height, distance albedo mottling on terrain, orange-roof desaturation, 16x anisotropy.
- Open: bounce light / SSAO / bloom (post agent output not integrated), Matterhorn raking-shadow crispness (needs heightfield ray-march or finer far cascade), headless weather shows rain/100% cloud (force clear), cloud shadows map-wide, z14 sat disk/net cost on first load.

## 2026-11-05: native look pass + shadow cache fix
- Resolve cost: swapping Filmic for ACES gave the same 2.4 ms headless reading, so the LUT is free; the number is a headless-timer artifact.
- Shadow cache: cascade depth range (`zn`/`zf` in `fit_cascades`) followed the camera continuously, so every key changed each frame ("+0 cached"). Now quantised. NYC (350 m): 26 -> 55-60 fps, Mexico City 1.8 km: 60 fps, headless 1280x720.
- Look: deeper zenith and narrower, bluer horizon band; fog colour taken from an elevated sky direction (blue, not grey), density 1/200 km; sun 5.2, bluer sky fill; dielectric environment Fresnel x0.3 (road glint). Matterhorn now close to Bevy's tone; Bevy still has crisper cast shadows and stronger relief.
- Open: far terrain LOD seam in Mexico City (blurry far tiles), orange roof saturation, bounce light, SSAO/bloom, GPU times for shadow4 at city scale.

## 2026-11-05: native resolve uses Blender Filmic + warm grade
- `ed_metal` resolve pass replaced ACES with Bevy's own Blender Filmic 64^3 LUT (`crates/ed_metal/assets/filmic_lut_rgba16f.bin`, extracted from `bevy_core_pipeline` `Blender_-11_12.ktx2`; same log2 -11..12 EV encode). Added a warm white balance (1.10/1.0/0.86) and a highlight tint; native exposure 1.0 -> 1.7.
- Headless Matterhorn: 61 fps, resolve reads 2.4 ms in the headless timer (was ~0.7 ms with ACES; recheck whether that is the LUT or timer wait). Needs a Mac check on the real window.

## 2026-11-05: native terrain shading port (first pass)
- Native terrain draws now get the baked DEM detail texture (`DrawItem::tex2`, texture slot 2, fed from `SatPool::cpu_detail`) and a `terrain_surf` branch in `world.metal`: detail-normal correction with wall fade, per-pixel rock/snow from slope + curvature + satellite brightness, strata bands, gully darkening, two-scale procedural micro relief.
- Terrain satellite/detail sampling uses a clamped sampler with the half-texel inset (previously `repeat`, which wrapped bilinear taps across tile edges).
- Fog density 1/130 km, weaker warm Mie lobe. Not yet ported: ragged layer edges, far-field colour steering, hex-tiled layer textures, far shadows from horizon maps.
- Needs a check: resolve/world GPU times looked higher (3-5 ms) in headless runs with this shader; re-measure.

## 2026-11-05: native far-terrain shadow cascade
- `ed_metal` now has a 5th shadow cascade (terrain casters only, 1.5 m proxies) reaching `terrain_shadow_far` = 20 km; `shadow_vis` falls through to it past the 4th cascade. Cost ~0.45 ms. Matterhorn at 2600 m, 1280x720: 78 fps, 1.2 M tris.
- `NotShadowCaster` is honoured by the native path (`DrawItem::cast_shadow`); the sky/ocean shells no longer darken roofs.
- New debug envs: `EW_NATIVE_NOPROXY`, `EW_NATIVE_DBG` (1 normals, 2 shadow vis, 3 ambient, 4 cascade, 5 ndl, 6-9 force cascade, 10 depth diff).
- Still open: cloud shadows over the whole map (native has no clouds; sky is clear), shadow cache never hits, compare with real photos.

Recent work, newest first, and what still needs checking. When an entry is verified on the Mac, fold the result into `ROADMAP.md` and trim it here. Plans live in `docs/plans/`; the roadmap says what's done and what's next.

---

## 2026-11-05: Game runs on the native Metal renderer (`EW_NATIVE=1`, macOS)

- `ed_metal::world::WorldRenderer` + `shaders/world.metal` draw the world into a `CAMetalLayer` view placed *below* Bevy's transparent window; Bevy keeps sim/input/audio/physics/UI. Bridge: `crates/ed_game/src/native.rs` (mirrors visible opaque `Mesh3d`s, CPU sphere cull, reverse-Z, sun from the `Sun` light).
- Native mode: Bevy 3D camera off, UI camera clears to transparent, `render_scale` skipped, meshes keep `MAIN_WORLD` data (`native::mesh_usage()`), window `transparent` + `PostMultiplied`.
- Zermatt, same spawn: Bevy path ~16 fps (GPU-bound) vs native ~110 fps (world pass ~7 ms, ~280 draws, 7.2 M tris). Look is basic (hemisphere light, fog, no shadows/atmosphere/clouds/textures yet); alpha-blended/tested materials, foliage, impostors and clouds are skipped for now.
- Next: heights into the clipmap, cluster ICB for buildings/trees, shadows, atmosphere/post/MetalFX (see `docs/plans/m2.md`).

## 2026-11-05: GPU-driven ICB probe (N0/N2 groundwork)

- `ed_metal` `icb_probe` (`cargo run --release -p ed_metal --bin icb_probe -- [instances] [seconds]`): a compute kernel frustum-culls instances and encodes `draw_indexed_primitives` into an indirect command buffer; the CPU issues one `executeCommandsInBuffer`. Per-pass GPU timers now cover render, compute and blit passes (`GpuTimers::attach/attach_compute/attach_blit`).
- Measured on M3 Pro (windowed 1280x720, unit cubes, 12 tris each): 1k -> 0.22 ms GPU; 10k -> 0.82 ms; 100k -> 4.8 ms (~125 fps, display-capped); 300k -> 14.1 ms (65-70 fps). Cull+encode is 0.1 ms at 100k and 0.35 ms at 300k, so the cull is not the limit. The main pass is ~45 ns per ICB draw, linear in draws: **many tiny draws are the cost, so real content needs clusters/merged draws, not one draw per object.**
- **Incident:** an earlier 1M-instance run was followed by the whole Mac crashing. Cause unconfirmed (no crash log tied to it). 1M was not rerun; stepped 1k -> 300k was clean. Do not run above 300k without being at the keyboard.
- `ed_metal` `terrain_probe` (`cargo run --release -p ed_metal --bin terrain_probe -- [levels] [seconds]`): `ed_gpu::clipmap` drawn with ONE instanced draw (heights from 6-octave noise in the vertex shader, no textures, no morph yet). M3 Pro windowed 1280x720: 4 levels (426k tris, 768 m) 1.9-2.0 ms GPU; 8 levels (819k tris, 12.3 km) 1.4-3.8 ms GPU, ~125 fps (display-capped). One pass, ~100 blocks, no per-tile objects. Next: real height source (tile heights in a GPU texture array), geomorphing at level seams, cluster cull for buildings/trees.
- `ed_gpu::clipmap` added (Haiku agent): 12 tests; 8 levels x 64-quad blocks = 819,200 tris covering 12.3 km radius. Blocks snap to their footprint, not 2x spacing; the shader must morph flagged edges. `cargo test -p ed_gpu`: 47 pass.
- Needs a Mac check: none beyond the above; MSL is compiled at runtime (offline `xcrun metal` still lacks the Metal Toolchain).

## 2026-11-05: GPU pass tooling, M2 plan
- `tools/gpu_passes.sh OUT_DIR [args]`: Metal System Trace of a headless run (with `EW_PERF` for real fps), `tools/gpu_sum.py` prints GPU occupancy and per-encoder shares. Needs Xcode's `xctrace`. Zermatt still: GPU 92% busy, ~61 ms/frame at 16 fps, so GPU-bound. No single hot pass: main opaque ~33%, shadow cascades ~21%, early prepass ~9% (excluding the headless-only surface copy). Absolute per-pass ms are not reported (intervals overlap); use shares.
- `crates/ed_gpu` (S1, core): pass graph with declared reads/writes, culling, transient lifetimes and alias plan; `Caps`; `PassTimes` (EMA per pass). No wgpu/Bevy dependency, 10 unit tests. wgpu backend (Bevy device, `timestamp_writes`) and the `ed_render` debug pass are next.
- **Priorities changed: native Metal first (N0..N7 in `docs/plans/m2.md` §4), no wgpu spike.** `crates/ed_metal` (objc2-metal): device/queue, `CAMetalLayer` on a winit window, runtime MSL compile, `GpuTimers` (ring of counter sample buffers at stage boundaries, resolved without stalls). `ed_metal_probe` renders at display rate and prints per-pass GPU ms (triangle ~0.15 ms). `ed_gpu` also gained `pool` (suballocator with deferred free), `cull` (instance/indirect structs, CPU reference, `shaders/cull.metal`), `clipmap` (terrain block generator). Offline `xcrun metal` needs `xcodebuild -downloadComponent MetalToolchain` (not run; runtime compile works).
- `docs/plans/m2.md`: M2 design rules and slices; native Metal spike starts after slice S1.
- The first 6-scene baseline (19-22 fps) was taken while a windowed game was open and is discarded. Do not run perf scenes while a game window is open. Clean baseline (quiet machine): still 16.0, cruise 14.5, spin 16.7, forest 15.2, lapaz 13.6, origin 15.2 fps (`target/perf_2026-11-05_clean/summary.txt`). The `EW_BISECT` base run that showed 72 fps had 0 tiles loaded; discard it.
- Not run: `EW_BISECT=hide:all` and `noshadow:all` hang the headless loop (frame counter runs away); `EW_GPU_WAIT` variant produced no rows. Use per-category hides instead. Windowed Mac pass still open.

---

## 2026-11-05: brighter midday
- Camera exposure `ev100` 14.2 -> 13.2 (`camera.rs`; `EW_EV100=<n>` overrides for A/B). Measured mean ground luminance at Zermatt 13:00 July: 102 (14.2), 120 (13.6), 137 (13.0). Sun illuminance already full `RAW_SUNLIGHT`, so this was exposure, not light. Check: snow/roofs not clipping, dusk and night still look right (exposure is global, so sunset gets brighter too), cloud shading.

---

## 2026-11-05: main menu look, lo-fi music, on-button pad prompts
- Main menu: blurred Matterhorn-at-sunset background (baked offline, `menu_assets/menu_bg.png`, shot with `--fly ... --hour 18.75 --date 2026-07-01 --no-hud`), Chewy title/buttons (`menu_assets/Chewy-Regular.ttf`, embedded). New flags: `--no-hud` (clean screenshots), `--menu-view home`.
- Music: `ed_audio::lofi` (72 BPM procedural lo-fi: Rhodes-ish chords, swung drums, bells, vinyl) plays only in `GameState::Menu` (`menu_music.rs`, fades with the master volume); the world soundscape sink is paused while the menu is up (`soundscape.rs` `gate_soundscape`).
- Controller: Home is now focusable (removed `pad_home`). The bottom hint bar is gone on button screens; prompts sit on the buttons (`menu.rs` `update_badges`): A on the focused one, B on Resume, LB/RB on the neighbouring tabs. The bar stays for the Map view, drop popup and in-game.
- Verified headless with `EW_PAD_SCRIPT` shots (Home, Settings). Check on the Mac: how the lo-fi mix sounds at default volume, whether the background wants to be brighter.

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
