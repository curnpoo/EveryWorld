// Offline render (Node 18+): drives each car through a scripted run (start-up,
// idle, revs, full-throttle pull, lift-off, cruise) with the real drivetrain
// and synthesizer, writes <id>.wav (16-bit stereo, 48 kHz) and prints level,
// pitch and CPU checks. Also writes <id>.controls.json: the exact control
// values fed to the synth every 1/60 s, with the events, so a port to another
// language can be fed the same controls and compared with the WAV. The
// randomness is seeded, so every run is the same.
//
//   node render.mjs [outDir] [carId ...]

import { writeFileSync, mkdirSync } from 'node:fs';
import { CARS, resolveCar } from './cars.js';
import { Drivetrain } from './drivetrain.js';
import { EngineVoice } from './synth.js';

const SR = 48000;
const CTRL = 1 / 60;           // the page updates the drivetrain at 60 Hz
let seed = 1;
Math.random = () => ((seed = (seed * 1103515245 + 12345) >>> 0) / 4294967296);   // start-up crank
const outDir = process.argv[2] || 'renders';
const only = process.argv.slice(3);

// gas, brake, rev over time (seconds).
function script(t) {
  if (t < 3) return { gas: 0, brake: 0, rev: false };
  if (t < 3.5) return { gas: 0, brake: 0, rev: true };
  if (t < 4.6) return { gas: 0, brake: 0, rev: false };
  if (t < 6.2) return { gas: 0, brake: 0, rev: true };     // hold into the limiter
  if (t < 7.5) return { gas: 0, brake: 0, rev: false };
  if (t < 17) return { gas: 1, brake: 0, rev: false };      // full pull
  if (t < 20) return { gas: 0, brake: 0, rev: false };      // lift: overrun
  if (t < 26) return { gas: 0.3, brake: 0, rev: false };    // cruise
  if (t < 30) return { gas: 0, brake: 0.5, rev: false };    // brake to a stop
  return { gas: 0, brake: 0, rev: false };
}
const LEN = 32;

function wav(L, R) {
  const n = L.length, buf = Buffer.alloc(44 + n * 4);
  buf.write('RIFF', 0); buf.writeUInt32LE(36 + n * 4, 4); buf.write('WAVE', 8);
  buf.write('fmt ', 12); buf.writeUInt32LE(16, 16); buf.writeUInt16LE(1, 20); buf.writeUInt16LE(2, 22);
  buf.writeUInt32LE(SR, 24); buf.writeUInt32LE(SR * 4, 28); buf.writeUInt16LE(4, 32); buf.writeUInt16LE(16, 34);
  buf.write('data', 36); buf.writeUInt32LE(n * 4, 40);
  for (let i = 0; i < n; i++) {
    buf.writeInt16LE(Math.max(-32767, Math.min(32767, Math.round(L[i] * 32767))), 44 + i * 4);
    buf.writeInt16LE(Math.max(-32767, Math.min(32767, Math.round(R[i] * 32767))), 46 + i * 4);
  }
  return buf;
}

// Strongest frequency between lo and hi Hz in a window (plain DFT on a
// handful of bins is plenty for a sanity check).
function peakHz(x, start, len, lo, hi) {
  let best = 0, bestHz = 0;
  for (let f = lo; f <= hi; f += 2) {
    let re = 0, im = 0;
    const w = 2 * Math.PI * f / SR;
    for (let i = 0; i < len; i++) { const v = x[start + i]; re += v * Math.cos(w * i); im += v * Math.sin(w * i); }
    const m = re * re + im * im;
    if (m > best) { best = m; bestHz = f; }
  }
  return bestHz;
}

mkdirSync(outDir, { recursive: true });
console.log('car            peak   rms   mid    idleHz(exp)   cpu×rt  maxRpm  topKmh gears');
for (const c of CARS) {
  if (only.length && !only.includes(c.id)) continue;
  const car = resolveCar(c);
  const dt = new Drivetrain(car.engine);
  dt.intensity = 0.75;
  seed = 1;
  const voice = new EngineVoice(SR, car, 7);
  voice.setUser(0.75, 0.5);   // character 0.75, crackle 0.5
  const trace = [];
  dt.start();
  const n = LEN * SR, L = new Float32Array(n), R = new Float32Array(n);
  const chunk = Math.round(SR * CTRL);
  let bangs = 0;
  let v = 0, a = 0, maxRpm = 0, topV = 0, maxGear = 1, nan = false;
  const t0 = process.hrtime.bigint();
  for (let i = 0; i < n; i += chunk) {
    const t = i / SR;
    const s = script(t);
    const sim = dt.simulate(CTRL, s.gas, s.brake);
    v = sim.v; a = sim.a;
    const o = dt.update(CTRL, { speed: v, accel: a, throttle: s.gas, rev: s.rev });
    maxRpm = Math.max(maxRpm, o.rpm); topV = Math.max(topV, v); maxGear = Math.max(maxGear, o.gear);
    const frame = { t: +t.toFixed(4), rpm: o.rpm, load: o.load, overrun: o.overrun, cut: o.cut, starter: o.starter, shaft: o.shaft, horn: o.horn, pullAway: o.pullAway, clicks: [], events: [] };
    for (const amt of dt.clicks.splice(0)) { voice.click(amt); frame.clicks.push(amt); }
    for (const ev of dt.events.splice(0)) { voice.event(ev.type, ev.strength); bangs++; frame.events.push([ev.type, ev.strength]); }
    voice.setParams(o);
    trace.push(frame);
    const m = Math.min(chunk, n - i);
    voice.render(L.subarray(i, i + m), R.subarray(i, i + m), m);
  }
  const cpu = Number(process.hrtime.bigint() - t0) / 1e9 / LEN;
  let peak = 0, sum = 0, mid = 0, hpX = 0, hpY = 0;
  const kHp = Math.exp(-2 * Math.PI * 80 / SR);
  for (let i = 0; i < n; i++) {
    if (!Number.isFinite(L[i]) || !Number.isFinite(R[i])) { nan = true; L[i] = R[i] = 0; }
    peak = Math.max(peak, Math.abs(L[i]), Math.abs(R[i])); sum += L[i] * L[i];
    hpY = kHp * (hpY + L[i] - hpX); hpX = L[i]; mid += hpY * hpY;
  }
  // Idle pitch check: firing frequency = rpm / 60 × cylinders / 2.
  const cyl = car.firing.length;
  const expHz = car.engine.idle / 60 * cyl / 2;
  const mono = new Float32Array(SR);
  for (let i = 0; i < SR; i++) mono[i] = L[Math.round(2 * SR) + i] + R[Math.round(2 * SR) + i];
  const got = peakHz(mono, 0, SR / 2, Math.max(10, expHz * 0.4), expHz * 2.6);
  writeFileSync(`${outDir}/${c.id}.wav`, wav(L, R));
  writeFileSync(`${outDir}/${c.id}.controls.json`, JSON.stringify({ sampleRate: SR, controlRate: 60, seed: 7, character: 0.75, crackle: 0.5, frames: trace }));
  console.log(
    c.id.padEnd(14), peak.toFixed(2).padStart(5), Math.sqrt(sum / n).toFixed(3).padStart(6), Math.sqrt(mid / n).toFixed(3).padStart(6),
    `${got}(${expHz.toFixed(0)})`.padStart(12), cpu.toFixed(3).padStart(7),
    Math.round(maxRpm).toString().padStart(7), (topV * 3.6).toFixed(0).padStart(6), String(maxGear).padStart(4),
    nan ? '  NaN!' : '', ' bangs=' + bangs
  );
}
