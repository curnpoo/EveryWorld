// worklet.js — runs the engine synthesizer on the audio thread.
import { EngineVoice, warmUp } from './synth.js';

class EngineProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.voice = null;
    this.blocks = 0; this.slow = 0; this.worst = 0;
    this.port.onmessage = e => {
      const m = e.data;
      if (m.type === 'warm') {
        // Before the voice exists: compile every sound path now, silently.
        this.port.postMessage({ type: 'warm', ms: warmUp(sampleRate, m.cars) });
      } else if (m.type === 'car') {
        if (this.voice) this.voice.configure(m.car);
        else this.voice = new EngineVoice(sampleRate, m.car, m.seed);
      } else if (!this.voice) {
        return;
      } else if (m.type === 'params') {
        this.voice.setParams(m);
      } else if (m.type === 'user') {
        this.voice.setUser(m.character, m.crackle);
      } else if (m.type === 'mix') {
        this.voice.setMix(m.engine, m.boost, m.pops);
      } else if (m.type === 'click') {
        this.voice.click(m.amt);
      } else if (m.type === 'bang') {
        this.voice.bang(m.amt);
      } else if (m.type === 'event') {
        this.voice.event(m.kind, m.amt);
      }
    };
  }

  process(inputs, outputs) {
    const out = outputs[0];
    if (!this.voice || !out || !out[0]) return true;
    const t0 = Date.now();
    this.voice.render(out[0], out[1] || out[0], out[0].length);
    // Health: blocks that took longer than they last (128 frames: 2.7 ms at
    // 48 kHz, 4 ms at 32 kHz), reported every couple of seconds for the
    // Compatibility list.
    const d = Date.now() - t0;
    if (d >= 128000 / sampleRate) { this.slow++; if (d > this.worst) this.worst = d; }
    // Boost for the gauge, about 30 times a second.
    if (this.blocks % 12 === 0) this.port.postMessage({ type: 'boost', v: this.voice.boost });
    if (++this.blocks >= 750) {
      this.blocks = 0;
      this.port.postMessage({ type: 'health', slow: this.slow, worst: this.worst });
    }
    return true;
  }
}

registerProcessor('engine', EngineProcessor);
