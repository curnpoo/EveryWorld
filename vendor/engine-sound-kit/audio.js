// audio.js — Web Audio around the synthesizer:
//   engine (AudioWorklet, or ScriptProcessor fallback) → compressor (tames
//   peaks on car speakers) → master volume → soft clipper → speakers.

import { EngineVoice, warmUp } from './synth.js';

function clipCurve() {
  const n = 2049, c = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const x = (i / (n - 1)) * 2 - 1, a = Math.abs(x);
    const y = a < 0.85 ? a : 0.85 + 0.13 * Math.tanh((a - 0.85) / 0.13);
    c[i] = Math.sign(x) * y;
  }
  return c;
}

// Android phones (not the Tesla: there 32 kHz made the sound stutter and
// cut out; not iPhone, where a rate other than the speaker's only adds a
// converter) run the synthesizer at a lower rate. A phone is several times
// slower, and an audio thread that runs late makes Chrome on Android grow
// its output buffer, which stays grown. At 32 kHz each block has two and a
// half times the headroom; the highest sounds (turbo whistles, up to 10 kHz)
// still fit.
const PHONE_RATE = 32000;

export class EngineAudio {
  constructor({ lowRate = false } = {}) {
    this.lowRate = lowRate;
    this.errors = [];      // audio thread failures, for diagnostics
    this.ctx = null;
    this.node = null;
    this.voice = null;     // fallback only: the synth on the main thread
    this.kind = '';
    this.volume = 0.8;
    this.ready = null;     // prepare() promise
    this.warmMs = null;    // how long the warm-up took
    this.health = { slow: 0, worst: 0 };
    this.boost = 0;                        // turbo boost 0..1, for the gauge   // audio blocks that ran late (worklet)
  }

  static supported() { return !!(window.AudioContext || window.webkitAudioContext); }

  // Sets up the audio graph and warms the synthesizer up (warmUp in
  // synth.js) before anything is heard, so the first start-up, bang or
  // blow-off doesn't stall the audio while the browser compiles code. Safe to
  // call before any tap: the context waits, silent, until start(). cars:
  // resolved cars, for the warm-up to play through.
  prepare(car, cars) {
    if (!this.ready) this.ready = this._prepare(car, cars).catch(err => { this.ready = null; throw err; });
    return this.ready;
  }

  async _prepare(car, cars) {
    this._context();
    if (!this.node) await this._build(car, cars);
  }

  _context() {
    if (this.ctx) return;
    const AC = window.AudioContext || window.webkitAudioContext;
    try {
      this.ctx = new AC(this.lowRate ? { latencyHint: 'interactive', sampleRate: PHONE_RATE } : { latencyHint: 'interactive' });
    } catch {
      this.ctx = new AC({ latencyHint: 'interactive' });   // that rate isn't offered
    }
    const comp = this.ctx.createDynamicsCompressor();
    comp.threshold.value = -14; comp.knee.value = 10; comp.ratio.value = 4;
    comp.attack.value = 0.004; comp.release.value = 0.15;
    this.master = this.ctx.createGain();
    this.master.gain.value = 0;
    const clip = this.ctx.createWaveShaper();
    clip.curve = clipCurve();
    comp.connect(this.master); this.master.connect(clip); clip.connect(this.ctx.destination);
    this.input = comp;
  }

  // Must first run inside a tap/click. Waits for prepare() (started now if
  // it wasn't already) before the audio runs: resuming while the warm-up
  // still had the audio thread busy left Chrome with a bigger output buffer
  // for good (about 130 ms of latency instead of 43). The tap still counts
  // once the warm-up is done (Chrome remembers the page was interacted with).
  async start(car, cars) {
    this._context();
    if (!this.unlocked) {
      // Wake the audio up in the same tap (iOS/Safari).
      const b = this.ctx.createBuffer(1, 1, this.ctx.sampleRate);
      const s = this.ctx.createBufferSource(); s.buffer = b; s.connect(this.ctx.destination); s.start();
      this.unlocked = true;
    }
    await this.prepare(car, cars);
    if (this.ctx.state !== 'running') await this.ctx.resume();
    this.setCar(car);
    this._fadeTo(this._gain(this.volume), 0.25);
  }

  // The real voice is made after the warm-up, never before: one made earlier
  // would still be laid out the way V8 first saw it and stall on first use.
  async _build(car, cars) {
    const ctx = this.ctx;
    const seed = (Math.random() * 1e9) | 0;
    // The warm-up keeps the audio thread busy for a moment; if the output is
    // already playing then (a page that has been tapped starts its audio
    // running), Chrome grows the output buffer to cover it and keeps it:
    // the sound ends up over half a second behind for good. Pause it first;
    // start() resumes it once the warm-up is done.
    if (ctx.state === 'running') { try { await ctx.suspend(); } catch { /* can't: carry on */ } }
    if (ctx.audioWorklet && window.AudioWorkletNode) {
      try {
        await ctx.audioWorklet.addModule(new URL('./worklet.js', import.meta.url));
        const node = new AudioWorkletNode(ctx, 'engine', { numberOfInputs: 0, outputChannelCount: [2] });
        // The synthesizer threw on the audio thread: the sound stops for good.
        node.onprocessorerror = e => this.errors.push({ at: Date.now(), what: 'audio thread error', msg: String(e && e.message || e) });
        const warmed = new Promise(resolve => {
          node.port.onmessage = e => {
            const m = e.data;
            if (m.type === 'warm') { this.warmMs = m.ms; resolve(); }
            else if (m.type === 'boost') this.boost = m.v;
            else if (m.type === 'health') this.health = m;
          };
          // Never hang the start button if the reply doesn't come.
          setTimeout(resolve, 15000);
        });
        this.node = node;
        this.post = m => node.port.postMessage(m);
        this.kind = 'AudioWorklet';
        this.post({ type: 'warm', cars });
        this.post({ type: 'car', car, seed });
        node.connect(this.input);
        await warmed;
        return;
      } catch (err) {
        console.warn('AudioWorklet failed, using ScriptProcessor', err);
        this.node = null;
      }
    }
    this.warmMs = warmUp(ctx.sampleRate, cars);
    this.voice = new EngineVoice(ctx.sampleRate, car, seed);
    this.node = ctx.createScriptProcessor(1024, 0, 2);
    this.node.onaudioprocess = e => {
      const b = e.outputBuffer;
      this.voice.render(b.getChannelData(0), b.getChannelData(1), b.length);
      this.boost = this.voice.boost;
    };
    this.post = m => {
      const v = this.voice;
      if (m.type === 'car') v.configure(m.car);
      else if (m.type === 'params') v.setParams(m);
      else if (m.type === 'user') v.setUser(m.character, m.crackle);
      else if (m.type === 'mix') v.setMix(m.engine, m.boost, m.pops);
      else if (m.type === 'click') v.click(m.amt);
      else if (m.type === 'bang') v.bang(m.amt);
      else if (m.type === 'event') v.event(m.kind, m.amt);
    };
    this.kind = 'ScriptProcessor';
    this.node.connect(this.input);
  }

  setCar(car) { if (this.post) this.post({ type: 'car', car }); }
  params(p) { if (this.post) this.post({ type: 'params', ...p }); }
  user(character, crackle) { if (this.post) this.post({ type: 'user', character, crackle }); }
  mix(engine, boost, pops) { if (this.post) this.post({ type: 'mix', engine, boost, pops }); }
  click(amt) { if (this.post) this.post({ type: 'click', amt }); }
  bang(amt) { if (this.post) this.post({ type: 'bang', amt }); }
  event(kind, amt) { if (this.post) this.post({ type: 'event', kind, amt }); }

  _gain(v) { return Math.pow(Math.max(0, Math.min(1, v)), 1.6) * 1.4; }

  setVolume(v) {
    this.volume = v;
    if (this.ctx && this.running) this._fadeTo(this._gain(v), 0.05);
  }

  _fadeTo(g, s) {
    const p = this.master.gain, t = this.ctx.currentTime;
    p.cancelScheduledValues(t);
    p.setValueAtTime(p.value, t);
    p.linearRampToValueAtTime(g, t + s);
    this.running = g > 0;
  }

  stop() {
    if (!this.ctx) return;
    this._fadeTo(0, 0.3);
  }

  resume() { if (this.ctx && this.ctx.state !== 'running') this.ctx.resume(); }

  // How far behind the sound is right now (ms): audio made now is heard once
  // the output reaches it. null if the browser can't say.
  delayMs() {
    const c = this.ctx;
    if (!c || c.state !== 'running' || !c.getOutputTimestamp) return null;
    const ts = c.getOutputTimestamp();
    if (!ts || !ts.performanceTime) return null;
    const heard = ts.contextTime + (performance.now() - ts.performanceTime) / 1000;
    return Math.max(0, (c.currentTime - heard) * 1000);
  }

  // Throws the audio output away and starts a fresh one (its buffer back to
  // small), warming up only the car that's playing. Needs the page to have
  // been tapped before (it has: the engine is running).
  async rebuild(car) {
    await this.dispose();
    await this.start(car, [car]);
  }

  // Closes the audio output for good; prepare() or start() makes a new one.
  async dispose() {
    const old = this.ctx;
    this.ctx = null; this.node = null; this.post = null; this.ready = null; this.unlocked = false;
    this.voice = null; this.running = false;
    try { if (old) await old.close(); } catch { /* already closed */ }
  }
}
