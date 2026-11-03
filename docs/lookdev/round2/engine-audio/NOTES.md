# Round 2: engine sound kit port

The owner's engine sound kit (`vendor/engine-sound-kit`, the JS is the source of truth) is now the game's vehicle audio. The six `.js` files are unchanged.

- `crates/ed_engine_audio`: engine-free Rust port.
  - `voice.rs` is `synth.js` (`EngineVoice`), line by line.
  - `special.rs` is `special.js` (jet, pod racer, warp drive, steam train).
  - `data/cars.json` is generated from `cars.js` by `tools/gen_cars.mjs`. Nothing is retyped by hand.
  - `driver.rs` holds the parts of `drivetrain.js` the game lacks.
  - `mixer.rs` hosts several voices with distance gain and pan.
  - `analysis.rs` and `tests/parity.rs` check the port against the JS.
- `crates/ed_game/src/engine_audio.rs`: Bevy integration. One custom `Decodable` source streams the mixer on Bevy's (rodio's) audio thread. Controls go through a bounded lock-free queue at frame rate.
- `crates/ed_audio` (the old synth the owner called "too much like tin") is deleted. Nothing used it.

## Listen

| File | What it is |
|---|---|
| `<id>.js.mp3` | The kit's own `render.mjs` reference: a 32 s scripted drive (start-up, idle, revs, limiter, full pull, lift, cruise, brake). |
| `<id>.rust.mp3` | The Rust voice fed the same controls (`<id>.controls.json`). |
| `game/<id>.drive.mp3` | The Rust voice driven the way the game drives it: the game's own car model (6-speed automatic from `vehicles.rs`) through `driver::Driver`. 28 s: start-up, idle, full pull through the gears, lift, cruise, brake to a stop. |
| `game/<id>.flyby.mp3` | The car passing the listener 8 m away at ~100-150 km/h, through the game's mixer (distance gain, pan, air absorption). |

Cars: `mustang` (NA cross-plane V8), `huracan` (twin-turbo V10, the one matched to a recording), `gt3` (race flat-six), `f1` (race V6 turbo), `civic` (turbo inline-4, manual), `hayabusa` (motorcycle inline-4), `jet` (fighter jet).

Make your own:

```bash
cargo run --release -p ed_engine_audio --example render -- --list
cargo run --release -p ed_engine_audio --example render -- huracan out.wav            # game drive
cargo run --release -p ed_engine_audio --example render -- gt3 out.wav --flyby        # through the mixer
node vendor/engine-sound-kit/render.mjs ref/ gt3 huracan jet                           # JS references
cargo run --release -p ed_engine_audio --example compare -- ref/ out/ gt3 huracan jet # side by side, numbers
cargo run --release -p ed_engine_audio --example bench                                 # 6 voices through the mixer
```

## How close the port is

Every JS reference was replayed through the Rust voice: the same controls every 1/60 s, the same events, the same seed. At 16-bit, 11 of the 12 renders are **bit-identical** to the JS. The Mustang differs by 1 LSB in a single sample.

Why it matches this closely:
- State is f64, like JS numbers.
- Values are stored as f32 exactly where the JS uses a Float32Array (pulse tables, bank sums, turbine smoothing, pipe delay lines).
- Every expression keeps the JS evaluation order.
- The xorshift noise generator is integer math, so it is identical.
- Control blocks start counting at each `render()` call, as in the JS.

Whole 32 s render, mono:

| Vehicle | RMS Rust / JS | Correlation | 16-bit samples that differ (of 3,072,000) | CPU per voice, Rust | CPU, JS (render.mjs) |
|---|---|---|---|---|---|
| gt3 | 0.1253 / 0.1253 | 1.0000 | 0 (max 0 LSB) | 0.81% | 2.6% |
| mustang | 0.1528 / 0.1528 | 1.0000 | 1 (max 1 LSB) | 0.73% | 1.3% |
| huracan | 0.1523 / 0.1523 | 1.0000 | 0 (max 0 LSB) | 1.22% | 3.4% |
| f1 | 0.1193 / 0.1193 | 1.0000 | 0 (max 0 LSB) | 1.22% | 1.8% |
| civic | 0.1573 / 0.1573 | 1.0000 | 0 (max 0 LSB) | 0.77% | 1.9% |
| hayabusa | 0.1201 / 0.1201 | 1.0000 | 0 (max 0 LSB) | 0.66% | 1.1% |
| moped | 0.1355 / 0.1355 | 1.0000 | 0 (max 0 LSB) | 0.49% | 1.5% |
| semi | 0.1932 / 0.1932 | 1.0000 | 0 (max 0 LSB) | 0.73% | 1.9% |
| jet | 0.1217 / 0.1217 | 1.0000 | 0 (max 0 LSB) | 0.62% | 1.0% |
| steam | 0.1561 / 0.1561 | 1.0000 | 0 (max 0 LSB) | 0.35% | 0.7% |
| pod | 0.0917 / 0.0917 | 1.0000 | 0 (max 0 LSB) | 0.46% | 1.0% |
| warp | 0.1043 / 0.1043 | 1.0000 | 0 (max 0 LSB) | 0.95% | 1.6% |

**huracan**, per section of the scripted drive (Rust / JS):

| Section | RMS | Centroid (Hz) | Band-share diff | Correlation |
|---|---|---|---|---|
| idle | 0.1442 / 0.1442 (+0.00 dB) | 53 / 53 | 0.000 | 1.000 |
| rev | 0.1023 / 0.1023 (+0.00 dB) | 212 / 212 | 0.000 | 1.000 |
| limiter | 0.1606 / 0.1606 (-0.00 dB) | 148 / 148 | 0.000 | 1.000 |
| pull | 0.2184 / 0.2184 (-0.00 dB) | 201 / 201 | 0.000 | 1.000 |
| lift | 0.1714 / 0.1714 (-0.00 dB) | 79 / 79 | 0.000 | 1.000 |
| cruise | 0.0912 / 0.0912 (+0.00 dB) | 318 / 318 | 0.000 | 1.000 |
| brake | 0.0429 / 0.0429 (-0.00 dB) | 136 / 136 | 0.000 | 1.000 |

Firing pitch through the pull, one reading per second from 8 s (Hz, Rust/JS): 243/243 610/610 579/579 642/642 667/667 653/653 710/710 710/710 710/710

**mustang**, per section of the scripted drive (Rust / JS):

| Section | RMS | Centroid (Hz) | Band-share diff | Correlation |
|---|---|---|---|---|
| idle | 0.1119 / 0.1119 (+0.00 dB) | 54 / 54 | 0.000 | 1.000 |
| rev | 0.1224 / 0.1224 (+0.00 dB) | 166 / 166 | 0.000 | 1.000 |
| limiter | 0.1716 / 0.1716 (-0.00 dB) | 127 / 127 | 0.000 | 1.000 |
| pull | 0.2136 / 0.2136 (-0.00 dB) | 157 / 157 | 0.000 | 1.000 |
| lift | 0.0936 / 0.0936 (+0.00 dB) | 72 / 72 | 0.000 | 1.000 |
| cruise | 0.1421 / 0.1421 (-0.00 dB) | 88 / 88 | 0.000 | 1.000 |
| brake | 0.0548 / 0.0548 (-0.00 dB) | 65 / 65 | 0.000 | 1.000 |

Firing pitch through the pull, one reading per second from 8 s (Hz, Rust/JS): 224/224 393/393 434/434 481/481 444/444 406/406 461/461 432/432 469/469

**jet**, per section of the scripted drive (Rust / JS):

| Section | RMS | Centroid (Hz) | Band-share diff | Correlation |
|---|---|---|---|---|
| idle | 0.0235 / 0.0235 (-0.00 dB) | 1365 / 1365 | 0.000 | 1.000 |
| rev | 0.0264 / 0.0264 (+0.00 dB) | 1591 / 1591 | 0.000 | 1.000 |
| limiter | 0.1869 / 0.1869 (-0.00 dB) | 600 / 600 | 0.000 | 1.000 |
| pull | 0.1743 / 0.1743 (-0.00 dB) | 1487 / 1487 | 0.000 | 1.000 |
| lift | 0.1077 / 0.1077 (+0.00 dB) | 2098 / 2098 | 0.000 | 1.000 |
| cruise | 0.0701 / 0.0701 (-0.00 dB) | 2021 / 2021 | 0.000 | 1.000 |
| brake | 0.0532 / 0.0532 (-0.00 dB) | 2033 / 2033 | 0.000 | 1.000 |

Firing pitch through the pull, one reading per second from 8 s (Hz, Rust/JS): 63/63 61/61 51/51 51/51 80/80 93/93 57/57 58/58 58/58

CPU is fraction of one core (Xeon @ 2.3 GHz, this cloud container, release build). The JS column is `render.mjs`'s own figure under Node, which includes its drivetrain, so it overstates the voice alone a little.

### Automated test

`cargo test -p ed_engine_audio` (part of `cargo test --workspace --exclude ed_game`):

- `tests/parity.rs` replays `tests/data/<id>.controls.json` and compares the Rust render with `tests/data/<id>.ref.wav`. Those are short clips of the JS render: 0.5 s each at idle, on the rev limiter, mid-pull, and right after the lift-off (blow-off valve, pops, bangs).
- Cars: mustang, huracan, gt3, f1, civic, hayabusa, jet.
- Per window it checks:
  - loudness within 0.5 dB
  - spectral centroid within 3%
  - octave-band energy shares within 0.03
  - waveform correlation above 0.98
  - firing pitch within 2 Hz during the pull
- The tolerances leave room for libm differences on other platforms. Today everything passes at about 0.00 dB and correlation 1.000.
- Mutation check: weakening the blow-off level by a third makes the huracan and civic tests fail.
- Regenerate the fixtures after a kit update:
  ```bash
  node vendor/engine-sound-kit/render.mjs ref/ mustang huracan gt3 f1 civic hayabusa jet
  node crates/ed_engine_audio/tools/make_test_data.mjs ref/ mustang huracan gt3 f1 civic hayabusa jet
  node crates/ed_engine_audio/tools/gen_cars.mjs
  ```

Unit tests also cover:
- the catalog
- the RNG against node
- the driver: start-up, rev mapping, shift cut and `up`, `lift`, limiter, `stop`, jet spool
- the mixer: distance and pan, multi-voice mix and release, the soft clip curve

## Vehicle mapping

| Game vehicle | Kit sound | Why |
|---|---|---|
| Car (`1`) | `huracan` | The kit's best-tuned car (matched by ear and spectrum to a recording). `N` in the car cycles: huracan, gt3, mustang, f458, lfa, m3e90, civic, supra, sti, gtr, hellcat, f1. |
| Plane (`2`) | `jet` | As asked. `N` cycles jet, pod, warp. |
| Rocket ship | none yet | `VehicleSound::Rocket` is the TODO hook in `engine_audio.rs`. The kit has no rocket: add a `rocket` generator to `special.rs`, or drive the jet with the afterburner held lit. |

How the game drives a voice (`driver.rs`):

- **Rev mapping.** The game's car has one generic engine: idle 850, automatic upshift at 6,300. That range is mapped linearly onto the kit car's range, from its idle to its full-throttle shift point (`engine.shift[1]`). So the Huracán revs to 8,400 and the GT3 to 8,900, where the game shifts.
- **Load** = 0.12 + 0.88 × throttle on the gas, 0 off it.
- **Overrun** follows "off the gas above 1.25 × idle" with the kit's 0.12 s lag.
- **Gear changes become shifts**, using the kit car's gearbox type (`GEARBOX` in `cars.js`):
  - a shift sends `up` (throttle > 0.4) or `down` (sport mode)
  - the revs blend over the shift time
  - DCT and sequential gearboxes cut the ignition; manuals click the clutch
  - downshifts blip
- **Rev limiter**: spark cut and bounce at `engine.limiter`.
- **`lift`** fires when snapping off from high revs (blow-off valve on turbo cars, pops and bangs).
- **`stop`** fires on coming to rest.
- **Clutch slip** holds the revs up when pulling away. **Pull-away** softens the boom.
- **`start`**: a vehicle cranks and fires up (starter motor, flare, bark) when it spawns.
- **Shaft** = wheel rpm × the game's final drive (gearbox whine).
- **Turbines** (jet, pod, warp): spool = lerp(idle, redline, 0.62 × throttle + 0.45 × airspeed / 120 m/s), with the kit's spool-up and spool-down times.

## Runtime and performance budget

- **Blocks.** The mixer renders 128-frame blocks on the audio thread, and each voice applies controls every 32 samples, as the README describes. Rodio pulls interleaved samples from a ring of one block.
- **No allocation or locks on the audio path.**
  - Every voice slot (8) is built up front.
  - A new vehicle reuses a slot with `EngineVoice::reset`, which keeps and clears the pipe buffers.
  - `configure()` writes fixed-size tables, so switching cars on the audio thread is safe.
  - Commands use a bounded crossbeam channel (`try_send` / `try_recv`).
  - There is no JIT, so the JS `warmUp()` has no Rust counterpart.
- **Audible budget.** Up to `MAX_AUDIBLE = 6` vehicles get a voice: the loudest after distance attenuation. Farther ones are released, and their drivers keep running so they come back in the right state.
- **Six voices in the mixer:** 162 us per 128-frame block, which is 6.1% of the 2.67 ms budget (about 6% of one core). Worst block 0.65 to 1.95 ms across runs on this shared container (`--example bench`).
- **Per voice:** 0.35% (steam) to 1.2% (huracan, f1) of a core. That is 1.5x to 3x cheaper than the JS.
- **In the game.** Checked by running under Xvfb with ALSA's null device, spawning a car and a plane with xdotool. Bevy's audio thread pulled the mixer the whole time.
  - The stats log (`engine audio: N voices, ...`, every 10 s) showed 1 to 2 voices at 2.3% to 4.7% of the block budget, in a debug build.
  - The workspace now builds `ed_engine_audio` at opt-level 3 even in dev.
  - Worst blocks there were higher (4 to 22 ms) because lavapipe software rendering kept every core busy. That container can't tell you the real worst case. Check the log on the Mac.
- **Spatialization:**
  - inverse-distance gain (full inside 10 m, fades out by 900 m)
  - equal-power pan from the camera's right vector, capped at 75%
  - the voice's own stereo (pipes left and right) up close, narrowing to a panned point source with distance
  - a one-pole air-absorption low-pass from 20 kHz down to 1.2 kHz with distance
  - the listener is the camera (the chase cam sits ~9.5 m behind the car, so the driven car plays at full level)
- **Output stage** (from `audio.js`): compressor (-14 dB threshold, 10 dB knee, 4:1, 4 ms / 150 ms), then master volume (0.8 with the kit's v^1.6 × 1.4 curve), then the kit's soft clipper.

## Known gaps

- **No Doppler.** Pitch doesn't bend on a drive-by. It would need a variable-rate resampler per voice, or bending the voice's rpm by the closing speed, which is cheap and probably good enough.
- **Fixed 48 kHz.** Rodio resamples to the device rate if it differs, which costs a little CPU.
- **Compressor makeup gain.** Web Audio's `DynamicsCompressor` adds automatic makeup gain and has look-ahead. The port's compressor has neither, so the mix is a little quieter and less squashed than the kit page at the same volume.
- **One engine model in the game.** Every kit car rides on the game's single 6-speed automatic (850 to 6,300 rpm) through the rev mapping, so shift points and the number of gears are the game's, not the real car's. If the game gets per-car gearing later, set `GameEngine` to match (or port `Drivetrain` fully, which the kit offers).
- **No reverse or neutral revving.** The game car has neither. The kit's "rev on the spot" (Rev button) has no key.
- **The plane is a propeller plane** in the physics, but it sounds like the fighter jet, as asked. A prop sound would be a new `special.rs` generator.
- **Rockets are silent** (TODO hook above).
- **Untested by ear on real hardware.** There is no sound card here: everything was checked through offline renders, numbers and the null-device run. Driving in and out of a car with `E` was not exercised in the headless run, because xdotool couldn't reliably seat the player. The driver path is covered by unit tests and the `render` example.

## Also fixed (found while testing)

Avian 0.8's `Forces` query data already holds mutable access to the body's velocities. Queries that also asked for `&LinearVelocity` / `&mut LinearVelocity` panicked as soon as the app started outside `--fly` mode:
- `car_physics` and `plane_physics` in `vehicles.rs`
- `player.rs`'s movement system

They now read and write the velocities through `Forces`. The `player.rs` change is its own commit, so it is easy to drop if another session fixes it differently.
