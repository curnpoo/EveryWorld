# Sound notes (from the original project)

## Signal path (synth.js)

crank angle → cylinder firings (the car's firing order) → exhaust pressure
pulse + combustion noise per bank (after header delay) → turbine smoothing →
X/H-pipe crossover → tailpipe waveguide (delay line + lossy inverting
reflection) panned to stereo → body resonances → muffler low-pass → plus
intake roar, valvetrain, turbo whistle/blow-off, supercharger/gearbox whine,
afterfire pops → DC block → saturation. Driven by smoothed rpm/load/overrun/
cut/starter/shaft from the drivetrain.


## Audio performance (keep the sound from cutting out)

Tried on the Tesla too (2026-10-21) and reverted the same day: at 32 kHz
the Tesla's main loop stuttered (50–300 ms gaps, dash redraws up to 75 ms)
and each self-reset cut the sound out; the user heard glitching and drop-
outs every second or two. The Tesla stays at 48 kHz with no self-reset. Its
drive reports did show the sound going ~100 → ~280 ms behind after a burst
of late blocks mid-drive, and main-loop gaps of 50–200 ms while driving
(40 ms at the start): the MCU is the bottleneck; lighten the main thread
(dash redraws) before touching its audio again. An
iPhone report in the car showed outputLatency 634 ms: audio routed to
Bluetooth (the car), not the app.
Phones (UA says mobile; the Tesla says desktop Linux, so it is untouched):
the AudioContext runs at 32 kHz (`PHONE_RATE` in audio.js). On a phone the
48 kHz synth ran late, and Chrome on Android then grows its output buffer
and keeps it grown: the user heard the sound about a second behind, in every
mode. 32 kHz gives each 128-frame block 2.5× the headroom (worst block
16% → 7% of its time on the dev box). Compatibility shows "behind right
now" (`EngineAudio.delayMs`, from getOutputTimestamp); if it stays over
300 ms for 3 s, Android rebuilds the audio output (`rebuild`, warm-up of the
current car only, ~0.3 s gap, at most once a minute; not iOS, which only
starts audio from a tap).

Any AudioContext made after the page has been tapped starts *running*, and
the warm-up then blocks its audio thread for ~0.7 s, so Chrome grows the
output buffer for good (measured 650 ms behind in headless Chromium). The
first one is made before any tap, so it was fine. `_build` now suspends the
context before the warm-up; start() resumes it. iPhone (a user's diagnostics:
fast at first, ~1 s behind after switching apps and back, nothing visible to
the page): leaving the page now stops the engine and disposes the audio;
coming back prepares a fresh one (`visibilitychange` in app.js). Diagnostics
(Settings → Send diagnostics) land in data/diag/.

The Tesla's CPU (Intel Atom on a 2020 Model 3) has 2.7 ms per 128-sample
block. Three things used to blow that, all fixed in `synth.js`:

- **First-time compiling.** V8 compiles the per-sample loop from what it has
  seen; the first bang, blow-off, start-up etc. made it recompile (tens of ms
  of slow audio). `warmUp()` plays every path silently in the worklet at page
  load (`audio.prepare`), before the real voice exists; Start waits for it.
  New sound feature → add it to `coverCars()` features / the warm-up tour.
- **Garbage.** Calls V8 doesn't inline box their numbers (~10 MB/s of garbage,
  ~10 collections a second). The hot filters, pipes and the noise generator
  are written out by hand in `render()`; nothing in the loop allocates.
- **Changing object layouts.** Slot/record classes start number fields at -0
  (stored as a double from the start) and live in fixed pools; never push
  new object shapes or splice arrays inside `render()`.

Check a change: `node tools/bench.mjs ref /tmp/ref` before, `cmp /tmp/ref`
after (must say ALL IDENTICAL unless the sound is meant to change), and
`node tools/bench.mjs perf huracan --warm` for timings. The Compatibility list
shows the warm-up time and any late audio blocks on the device; GPS logs
record the late-block count as `late`.


## Huracán matched to a recording (2026-10-21)
Tuned against an onboard clip of the UGR 1R build (spectra compared at matched rpm/load,
normalized to the 315-1250 Hz band). Real car: mid-heavy roar, steep fall above 2 kHz;
whistle sweeps ~2.7k -> 11k+ while spooling (~2 s), loudest low in the sweep, then gone;
blow-off only on lift (not on DCT upshifts), ~0.3 s, centred 1-1.6 kHz, louder than the
engine in the mids briefly. New optional sound keys (other cars unaffected):
`toneHz` (final lowpass on the engine, also takes the turbo hiss), `turbo.hissHz/hissQ`,
`turbo.whistleFade` (sweep-with-spool whistle), `turbo.bovHz/bovQ/bovAttack/bovShift`.
Follow-up from driving it: kept brighter than the recording (toneHz 6000) since the clip
sounded muffled through the car; BOV shortened (decay 0.09 s); `turbo.whistleIdle` adds
the faint idle whine / spin-down; `turbo.whistlePulse` makes it pulse with the firing near idle; `turbo.rush` adds bright compressor air only on hard high-boost pulls (user liked the old brightness there); `turbo.whistleTwin` adds the second turbo a few % higher (clip: 1868 + 1964 Hz at light load, beating). All turbo cars: one vent per boost build (`bovArmed`
re-arms only after load > 0.4 with boost > 0.3), so a lift can't pssh twice.
Character barely changes the Huracán now (its tone filter sets the top end).
