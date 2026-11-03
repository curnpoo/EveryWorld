# Engine sound kit

Procedural engine sounds for the browser: 30 vehicles (NA, turbo, race, and
a few fun ones: jet, steam train, pod racer, warp drive), synthesized live
from crank angle up, with no samples. Built and tuned for a Tesla-dashboard
web app, handed over here for use in a game.

This README is written for the developer (or AI assistant) integrating it.
Read "Driving it from a game" and "Performance rules" before changing anything.

## Not a browser? Port it

The code is JavaScript for the browser's Web Audio. A native game (Rust, C++,
Unity, …) can't run it as is; it has to be ported, and `synth.js` is a plain
sample-by-sample DSP loop (about 1,000 lines, no libraries, biquad filters,
delay lines, a seeded noise generator) that ports line by line. Order of work:

1. `node render.mjs out/ gt3 huracan` writes reference WAVs and
   `<id>.controls.json` (what the game would send every 1/60 s).
2. Port `EngineVoice` (`synth.js`) and the car data (`cars.js`; the `sound` and
   `engine` objects are plain data, so generate Rust/JSON from it rather than
   retyping). Port `special.js` only if the jet / pod / warp / steam sounds
   are wanted.
3. Feed your port the same controls and compare with the reference WAV:
   same loudness, same pitch as the revs climb, same blow-off and pops. Exact
   bit equality isn't needed (the noise generator and float order differ), but
   levels and spectra should match closely; the JS is the source of truth.
4. Port `drivetrain.js` only if the game has no gearbox model of its own; a
   game with real rpm, throttle and gear changes can send its own values to
   the voice (see "Driving it from a game").
5. Run the voice on the audio thread in blocks (the JS renders 128 frames at a
   time, and applies the controls every 32-sample block), and warm up / avoid
   allocation there as the performance rules below say. The per-voice cost in
   JS is 2–3% of a core; native should be far less.

## Files

| File | What it is |
| --- | --- |
| `synth.js` | `EngineVoice`: the DSP. Pure, no DOM, runs in an AudioWorklet, a ScriptProcessor, or Node. `warmUp()`. |
| `special.js` | The non-piston vehicles (jet, pod, warp, steam), used by `synth.js`. |
| `cars.js` | `CARS` (every vehicle: engine data + sound recipe), `DEFAULT_SOUND`, `GEARBOX`, `resolveCar()`. |
| `drivetrain.js` | Optional. `Drivetrain`: turns road speed + throttle into rpm, gear, shifts, load, overrun and events. |
| `worklet.js` | The AudioWorklet processor that hosts one `EngineVoice`. |
| `audio.js` | `EngineAudio`: the Web Audio graph (worklet → compressor → volume → soft clip), warm-up, ScriptProcessor fallback. |
| `demo.html` | A minimal page: pick a car, hold Space for throttle, B for brake, R to rev. Serve the folder over http(s) and open it. |
| `render.mjs` | Offline render in Node: a WAV per car from a scripted drive, plus the per-frame control values it used (`<id>.controls.json`). The reference for checking a port. |
| `SOUND-NOTES.md` | Design notes: signal path, what each sound key does, how cars were tuned. |

All files are plain ES modules with no dependencies. Keep them together
(`audio.js` loads `worklet.js` by URL next to itself; `worklet.js` imports
`synth.js`).

## How the sound is made

Per sample: crank angle → each cylinder fires in the car's firing order →
an exhaust pressure pulse plus combustion noise into that cylinder's bank,
after its header delay → turbine smoothing (turbo cars) → X/H-pipe crossover
→ tailpipe waveguides (a delay line with a lossy, inverting reflection),
panned to stereo → body resonances → muffler low-pass (opens with load) →
plus intake roar, valvetrain, turbo whistle and blow-off, supercharger and
gearbox whine, afterfire pops and bangs → DC block → saturation.

The character of each car comes from real data (cylinder count, firing
order, bank layout, header lengths, displacement, redline) plus a hand-tuned
`sound` recipe in `cars.js`. The Huracán twin-turbo was matched by ear and
by spectrum to a recording of the real car.

## Driving it from a game

Two layers. Use both, or just the first.

### 1. `EngineVoice`: you supply the engine state

Every frame (60 Hz is plenty; the voice smooths between updates), send:

```js
{
  rpm,       // crank rpm (idle … limiter)
  load,      // 0..1 combustion strength; ~0.12 + 0.88 × throttle when on throttle, 0 off it
  overrun,   // 0..1: 1 = closed throttle above idle (engine braking: darker drone, crackle)
  cut,       // 1 while the ignition is cut (rev limiter, upshift cut), else 0
  starter,   // 0..1 while cranking at start-up
  shaft,     // gearbox output rpm (gearbox whine); 0 when stopped
  horn,      // steam whistle (steam train only)
  pullAway,  // 0..1 gentle pull-away from a stop (softens the low boom); 0 is fine
  body       // 0..1 optional: 1 keeps engine braking at a light-cruise level instead of much quieter
}
```

And events as they happen:

- `event('up', strength)`: an upshift (strength = throttle 0..1). Turbo cars vent; some cars bang.
- `event('down', strength)`: a downshift (blip pops on some cars).
- `event('lift', strength)`: snapping off the throttle from high revs: the blow-off valve fires (turbo cars, once per boost build) plus a pop or bang.
- `event('start', strength)`: the start-up bark.
- `event('stop', 1)`: came to a stop (air-brake hiss on the semi).
- `click(amt)`: a manual gearbox's clutch/gear click. `bang(amt)`: a backfire on demand.

`voice.boost` (0..1) reads back the turbo boost, for a gauge.
`setUser(character, crackle)`: character 0..1 is stock to open pipes (0.75 is the default);
crackle 0..1. `setMix(engine, boost, pops)`: per-layer levels, 1 = as voiced (0..2).

### 2. `Drivetrain`: you supply speed and throttle

If the game has a car's speed and the player's throttle but no engine model,
`Drivetrain` does the rest: gears with realistic ratios and shift points
(sport or eco), shift timing, rev limiter, clutch slip pulling away, start-up
sequence, engine braking, kickdown, and the events above.

```js
const dt = new Drivetrain(car.engine);
dt.intensity = 0.75;   // how eagerly throttle shifts late (0..1)
dt.sport = true;       // hold low gears and high revs
dt.start();            // crank and fire up
// every frame:
const o = dt.update(seconds, { speed, accel, throttle, rev: false });   // speed m/s, accel m/s², throttle 0..1
audio.params({ rpm: o.rpm, load: o.load, overrun: o.overrun, cut: o.cut, starter: o.starter,
               shaft: o.shaft, horn: o.horn, pullAway: o.pullAway });
for (const amt of dt.clicks.splice(0)) audio.click(amt);
for (const ev of dt.events.splice(0)) audio.event(ev.type, ev.strength);
// o.gear, o.throttle, o.shifting for a dash
```

`dt.simulate(seconds, gas, brake)` is a simple built-in car physics model
(returns `{ v, a }`), handy for testing or for a game without its own.
`throttle: null` makes it estimate throttle from acceleration (that's the
GPS mode it was built for); a game should pass the real throttle.
`dt.setTopSpeed(mph)` shortens all ratios so top gear tops out at that speed
(`null` = the real car).

### 3. `EngineAudio`: the Web Audio wrapper

```js
const audio = new EngineAudio();
const cars = CARS.map(c => resolveCar(c));
audio.prepare(car, cars);              // at page load, before any tap: builds the worklet and warms it up silently
startButton.onclick = () => audio.start(car, cars);   // must be in a user gesture
audio.params(p); audio.event(type, amt); audio.click(amt);
audio.setCar(car); audio.setVolume(0..1); audio.mix(engine, boost, pops); audio.user(character, crackle);
audio.boost;                           // 0..1, from the worklet ~30×/s
audio.stop();                          // fades out
```

`resolveCar(c)` merges a car's `sound` over `DEFAULT_SOUND`; always pass
resolved cars to the voice.

### Several cars at once

One `EngineVoice` costs about 2–3% of a desktop CPU core at 48 kHz (much
more on a weak device). The wrapper hosts one voice. For opponents, host
extra voices in the same worklet (sum them into the output, with distance
gain and panning), and keep the count small, or give distant cars a simpler
treatment. Each voice needs its own `configure(car)`, `setParams` and events.

## Performance rules (learned the hard way)

The audio thread gets 2.7 ms per 128-sample block at 48 kHz. Miss it and
the sound clicks; miss it repeatedly and Chrome enlarges its output buffer
**for good** (the sound then lags the game by 100s of ms until a reload).

1. **Warm up before anything is heard.** V8 compiles the per-sample loop from
   what it has seen; the first bang or blow-off made it recompile mid-sound.
   `warmUp(sr, cars)` plays every code path silently; `EngineAudio.prepare()`
   does it in the worklet at load. New sound feature → add it to
   `coverCars()` / the warm-up tour in `synth.js`.
2. **Warm up with the AudioContext suspended.** A context created after the
   page has had a tap starts running; warming up while it runs grows the
   buffer permanently (measured: 650 ms behind). `audio.js` suspends it for
   the warm-up and resumes it after.
3. **Nothing in `render()` allocates.** The hot filters, pipes and noise are
   written out by hand; slot classes start number fields at `-0` and live in
   fixed pools. Don't add closures, array pushes or new object shapes there.
4. **Weak devices**: Android phones run it at 32 kHz (`lowRate`), 2.5× the
   headroom. On iPhone, keep the speaker's rate. When an iPhone page goes to
   the background, iOS pauses its audio, and the resumed audio can come back
   ~1 s late: dispose and rebuild the context on return.
5. **Keep the main thread light** on slow machines: heavy UI redraws delay the
   parameter updates.

## Adding or tuning a car

Copy an entry in `CARS` (`cars.js`): `engine` (idle, redline, limiter,
ratios, final drive, tire circumference, mass, power, gearbox, start-up) and
`sound` (pipes per bank with header delay and pan, pulse widths, noise,
muffler, body resonances, intake, turbo or supercharger blocks, pops, bangs,
drive and gain). `SOUND-NOTES.md` explains the keys and how the Huracán was
matched to a recording. Change one thing at a time and listen at idle, a full
pull, and a lift.
