// special.js — sound generators for vehicles that aren't piston engines:
// a fighter jet, a steam locomotive, a pod racer and a warp drive.
//
// Each is driven by the same smoothed values as the engine synthesizer
// (EngineVoice hands over its target: rpm, load, overrun, horn), where "rpm"
// means whatever turns: the turbine spool (jet, pod racer, warp core) or
// chuffs per minute (steam). Pure DSP, like synth.js.

const TAU = Math.PI * 2;
const BLOCK = 32;

function clamp(x, lo, hi) { return x < lo ? lo : x > hi ? hi : x; }
function lerp(a, b, t) { return a + (b - a) * t; }
function smoothstep(x) { x = clamp(x, 0, 1); return x * x * (3 - 2 * x); }
function coef(sr, seconds) { return 1 - Math.exp(-1 / (sr * Math.max(seconds, 1e-5))); }

class Biquad {
  constructor() { this.b0 = 1; this.b1 = 0; this.b2 = 0; this.a1 = 0; this.a2 = 0; this.z1 = 0; this.z2 = 0; }
  _set(b0, b1, b2, a0, a1, a2) { this.b0 = b0 / a0; this.b1 = b1 / a0; this.b2 = b2 / a0; this.a1 = a1 / a0; this.a2 = a2 / a0; }
  lowpass(sr, hz, q) {
    const w = TAU * clamp(hz, 10, sr * 0.45) / sr, c = Math.cos(w), al = Math.sin(w) / (2 * q);
    this._set((1 - c) / 2, 1 - c, (1 - c) / 2, 1 + al, -2 * c, 1 - al);
    return this;
  }
  bandpass(sr, hz, q) {
    const w = TAU * clamp(hz, 10, sr * 0.45) / sr, c = Math.cos(w), al = Math.sin(w) / (2 * q);
    this._set(al, 0, -al, 1 + al, -2 * c, 1 - al);
    return this;
  }
  process(x) {
    const y = this.b0 * x + this.z1;
    this.z1 = this.b1 * x - this.a1 * y + this.z2;
    this.z2 = this.b2 * x - this.a2 * y;
    return y;
  }
}

// Shared: smoothing of the drivetrain's values and the output stage.
class Base {
  constructor(sr, rng) {
    this.sr = sr; this.rng = rng;
    this.rpm = 0; this.load = 0; this.overrun = 0; this.horn = 0;
    this.kRpm = coef(sr, 0.03); this.kLoad = coef(sr, 0.05);
  }
  configure(car) {
    this.idle = car.engine.idle; this.redline = car.engine.redline;
    this.gain = car.sound.gain;
  }
  _follow(host) {
    const t = host.target;
    this.rpm += (t.rpm - this.rpm) * this.kRpm;
    this.load += (t.load - this.load) * this.kLoad;
    this.overrun += (t.overrun - this.overrun) * this.kLoad;
    this.horn += ((t.horn || 0) - this.horn) * this.kLoad;
  }
  _out(x, drive) { return Math.tanh(x * drive) * this.gain / Math.tanh(drive); }
}

// ── Fighter jet ─────────────────────────────────────────────────────────
// Fan and compressor whine rising with the spool, a roar that opens up with
// thrust, a buzz-saw edge near full power, and at full throttle the
// afterburner lights: a whump, then a deep tearing rumble full of crackle.
class Jet extends Base {
  constructor(sr, rng) {
    super(sr, rng);
    this.roarL = new Biquad(); this.roarR = new Biquad();
    this.rumL = new Biquad().lowpass(sr, 140, 0.7); this.rumR = new Biquad().lowpass(sr, 140, 0.7);
    this.abL = new Biquad().lowpass(sr, 380, 0.7); this.abR = new Biquad().lowpass(sr, 380, 0.7);
    this.crackBp = new Biquad().bandpass(sr, 2400, 0.9);
    this.ph1 = 0; this.ph2 = 0; this.ph3 = 0; this.shaft = 0;
    this.ab = 0; this.abLit = false; this.ign = 0; this.ignT = 0; this.crack = 0;
  }
  render(outL, outR, n, host) {
    const sr = this.sr, rng = this.rng, dt = BLOCK / sr;
    for (let i = 0; i < n; i++) {
      this._follow(host);
      const N = clamp(this.rpm / this.redline, 0, 1.05), load = this.load;
      if ((i % BLOCK) === 0) {
        const fc = 250 + 3800 * N * N;
        this.roarL.lowpass(sr, fc, 0.6); this.roarR.lowpass(sr, fc * 1.04, 0.6);
        const want = load > 0.64 && N > 0.5;   // afterburner at 60%+ throttle (load = 0.1 + 0.9·throttle), spooled
        if (want && !this.abLit && this.ab < 0.1) { this.ign = 1; this.ignT = 0; }
        this.abLit = want;
        this.ab += ((want ? 1 : 0) - this.ab) * Math.min(1, dt / (want ? 0.35 : 0.6));
      }
      const thrust = 0.05 + Math.pow(N, 2.2);
      const nL = rng.bi(), nR = rng.bi();
      let L = this.roarL.process(nL) * thrust * 1.3 + this.rumL.process(nL) * thrust * 2.2;
      let R = this.roarR.process(nR) * thrust * 1.3 + this.rumR.process(nR) * thrust * 2.2;
      // Whine: fan blade-pass with a beating sideband, and the compressor above it.
      const f1 = 900 + 2600 * N, f2 = 2200 + 5200 * N;
      this.ph1 += f1 / sr; this.ph1 -= Math.floor(this.ph1);
      this.ph2 += f2 / sr; this.ph2 -= Math.floor(this.ph2);
      this.ph3 += f1 * 1.06 / sr; this.ph3 -= Math.floor(this.ph3);
      const whine = 0.1 * (0.4 + 0.6 * N) * (Math.sin(this.ph1 * TAU) + 0.3 * Math.sin(this.ph3 * TAU))
        + 0.06 * (0.2 + 0.8 * N) * Math.sin(this.ph2 * TAU);
      // Buzz-saw: the fan tips go supersonic near full power.
      this.shaft += (60 + 160 * N) / sr; this.shaft -= Math.floor(this.shaft);
      const buzz = (2 * this.shaft - 1) * 0.07 * smoothstep((N - 0.72) / 0.28) * (0.4 + 0.6 * load);
      L += whine + buzz; R += whine * 0.95 + buzz;
      // Afterburner: deep tearing rumble plus crackle, lit with a whump.
      if (this.ab > 1e-3) {
        L += this.abL.process(rng.bi()) * this.ab * 5;
        R += this.abR.process(rng.bi()) * this.ab * 5;
        if (rng.next() < this.ab * 700 / sr) this.crack = 0.6 + 0.4 * rng.next();
        if (this.crack > 1e-3) {
          const c = this.crackBp.process(rng.bi()) * this.crack * 1.1;
          L += c; R += c * 0.8;
          this.crack *= Math.exp(-1 / (0.004 * sr));
        }
      }
      if (this.ign > 1e-3) {
        this.ignT += 1 / sr;
        const w = this.ign * (Math.sin(TAU * 38 * this.ignT) * 2.4 + rng.bi() * 0.8 * Math.exp(-this.ignT / 0.05));
        L += w; R += w;
        this.ign *= Math.exp(-1 / (0.35 * sr));
      }
      outL[i] = this._out(L, 1.3);
      outR[i] = this._out(R, 1.3);
    }
  }
}

// ── Pod racer ───────────────────────────────────────────────────────────
// Two open turbines, left and right, slightly out of tune with each other:
// a nasal, buzzing scream (a sawtooth through a resonance, chopped by a
// low buzz), a ragged roar, and electrical crackle that grows with thrust.
class Pod extends Base {
  constructor(sr, rng) {
    super(sr, rng);
    this.res = [new Biquad(), new Biquad()];
    this.roar = [new Biquad(), new Biquad()];
    this.zap = new Biquad().bandpass(sr, 3200, 1.2);
    this.ph = [0, 0]; this.bz = [0, 0]; this.lfo = 0; this.crack = 0; this.crackPan = 0;
  }
  render(outL, outR, n, host) {
    const sr = this.sr, rng = this.rng;
    for (let i = 0; i < n; i++) {
      this._follow(host);
      const N = clamp(this.rpm / this.redline, 0, 1.05), load = this.load, thrust = Math.pow(N, 1.8);
      this.lfo += 0.7 / sr; this.lfo -= Math.floor(this.lfo);
      const det = 0.012 * Math.sin(this.lfo * TAU);
      const out = [0, 0];
      for (let s = 0; s < 2; s++) {
        const Ns = N * (1 + (s ? -det : det));
        if ((i % BLOCK) === 0) {
          this.res[s].bandpass(sr, 1800 + 1600 * Ns, 2.2);
          this.roar[s].lowpass(sr, 400 + 3000 * Ns, 0.7);
        }
        this.ph[s] += (140 + 720 * Ns) / sr; this.ph[s] -= Math.floor(this.ph[s]);
        this.bz[s] += (45 + 160 * Ns) / sr; this.bz[s] -= Math.floor(this.bz[s]);
        const saw = 2 * this.ph[s] - 1;
        const chop = this.bz[s] < 0.5 ? 1 : 0.35;
        out[s] = this.res[s].process(saw) * (0.5 + 0.8 * thrust) * chop * 1.4
          + this.roar[s].process(rng.bi()) * thrust * chop * 1.6
          + saw * 0.05 * (0.3 + load);
      }
      // Electrical crackle.
      if (rng.next() < (15 + 90 * thrust) / sr) { this.crack = 0.5 + 0.5 * rng.next(); this.crackPan = rng.bi(); }
      if (this.crack > 1e-3) {
        const c = this.zap.process(rng.bi()) * this.crack * 1.4;
        out[0] += c * (1 - this.crackPan) * 0.6; out[1] += c * (1 + this.crackPan) * 0.6;
        this.crack *= Math.exp(-1 / (0.004 * sr));
      }
      outL[i] = this._out(out[0], 2.4);
      outR[i] = this._out(out[1], 2.4);
    }
  }
}

// ── Warp drive ──────────────────────────────────────────────────────────
// A deep, chorused core hum that climbs with speed, a faint shimmer on top,
// and on hard acceleration a jump: a rising whine and whoosh that ends in a
// deep boom.
class Warp extends Base {
  constructor(sr, rng) {
    super(sr, rng);
    this.ph = new Float64Array(10); this.lfo = 0;
    this.shim = new Biquad().bandpass(sr, 5500, 3);
    this.whoosh = new Biquad();
    this.jumpT = 9; this.cool = 9; this.prevLoad = 0; this.jPh = 0; this.boomT = 9;
  }
  event() {}
  render(outL, outR, n, host) {
    const sr = this.sr, rng = this.rng, dt = BLOCK / sr;
    const parts = [1, 2, 3, 4.02, 6.01], amps = [1, 0.6, 0.35, 0.22, 0.12];
    for (let i = 0; i < n; i++) {
      this._follow(host);
      const N = clamp(this.rpm / this.redline, 0, 1.05), load = this.load;
      if ((i % BLOCK) === 0) {
        this.cool += dt; this.jumpT += dt; this.boomT += dt;
        if (load > 0.75 && this.prevLoad <= 0.75 && this.cool > 5) { this.jumpT = 0; this.cool = 0; this.jPh = 0; }
        this.prevLoad = load;
        if (this.jumpT < 1.25) this.whoosh.bandpass(sr, 200 * Math.pow(30, this.jumpT / 1.2), 1.5);
        if (this.jumpT >= 1.2 && this.jumpT < 1.2 + dt * 1.5) this.boomT = 0;
      }
      this.lfo += 0.13 / sr; this.lfo -= Math.floor(this.lfo);
      const f0 = 30 + 55 * N, level = 0.35 + 0.4 * load + 0.25 * N;
      let L = 0, R = 0;
      for (let k = 0; k < 5; k++) {
        const d = 0.003 * Math.sin((this.lfo + k * 0.2) * TAU);
        this.ph[k] += f0 * parts[k] * (1 + d) / sr; this.ph[k] -= Math.floor(this.ph[k]);
        this.ph[k + 5] += f0 * parts[k] * (1 - d) / sr; this.ph[k + 5] -= Math.floor(this.ph[k + 5]);
        L += Math.sin(this.ph[k] * TAU) * amps[k];
        R += Math.sin(this.ph[k + 5] * TAU) * amps[k];
      }
      L *= level * 0.5; R *= level * 0.5;
      const sh = this.shim.process(rng.bi()) * (0.03 + 0.05 * N) * (0.6 + 0.4 * Math.sin(this.lfo * TAU * 3));
      L += sh; R -= sh;
      if (this.jumpT < 1.25) {
        const x = this.jumpT / 1.2, env = Math.sin(Math.PI * Math.min(1, x)) * 0.8 + 0.2 * x;
        this.jPh += f0 * 2 * Math.pow(16, x) / sr; this.jPh -= Math.floor(this.jPh);
        const j = Math.sin(this.jPh * TAU) * 0.25 * env + this.whoosh.process(rng.bi()) * 1.2 * env;
        L += j; R += j;
      }
      if (this.boomT < 1.5) {
        const b = Math.sin(TAU * 34 * this.boomT) * 1.8 * Math.exp(-this.boomT / 0.45) + rng.bi() * 0.3 * Math.exp(-this.boomT / 0.03);
        L += b; R += b;
      }
      outL[i] = this._out(L, 1.2);
      outR[i] = this._out(R, 1.2);
    }
  }
}

// ── Steam locomotive ────────────────────────────────────────────────────
// Four chuffs per turn of the driving wheels (two double-acting cylinders),
// louder with the regulator open; rod clank and rail joints that follow the
// speed; the air pump thumping at a standstill; a three-chime whistle on the
// Rev button; and a blow-down of steam on stopping.
class Steam extends Base {
  constructor(sr, rng) {
    super(sr, rng);
    this.puff = new Biquad().bandpass(sr, 1100, 0.8);
    this.body = new Biquad().lowpass(sr, 260, 0.7);
    this.hiss = new Biquad().bandpass(sr, 6000, 0.5);
    this.clank = new Biquad().bandpass(sr, 2800, 3);
    this.rail = new Biquad().bandpass(sr, 1800, 2);
    this.blow = new Biquad().bandpass(sr, 2500, 0.5);
    this.chime = [349, 440, 523].map(f => ({ f, ph: 0, bp: new Biquad().bandpass(sr, f, 30) }));
    this.chPh = 0; this.chN = 0; this.chEnv = 0; this.chAmp = 0; this.chT = 9; this.chTau = 0.1;
    this.rodPh = 0; this.clankEnv = 0;
    // Pending rail-joint clicks (seconds until each), oldest first; a fixed
    // array, so the per-sample code never sees it change type. At most a few
    // are ever pending (a joint every 18 m, each gone 15 ms after it sounds).
    this.railX = 0; this.clicks = new Float64Array(16); this.nClicks = 0; this.thunkPh = 0;
    this.pumpT = 0; this.pumpEnv = 0; this.pumpPh = 0;
    this.blowEnv = 0; this.hornT = 0;
  }
  configure(car) { super.configure(car); this.tire = car.engine.tire; }
  event(type) { if (type === 'stop') this.blowEnv = 1; }
  render(outL, outR, n, host) {
    const sr = this.sr, rng = this.rng;
    const accent = [1, 0.75, 0.9, 0.7];
    for (let i = 0; i < n; i++) {
      this._follow(host);
      const rate = Math.max(0, this.rpm) / 60;     // chuffs per second
      const N = clamp(this.rpm / this.redline, 0, 1.05), load = this.load;
      let L = 0, R = 0;
      // Chuffs.
      if (rate > 0.2) {
        this.chPh += rate / sr;
        if (this.chPh >= 1) {
          this.chPh -= 1;
          this.chN = (this.chN + 1) % 4;
          this.chT = 0; this.chAmp = (0.15 + 0.85 * load) * accent[this.chN] * (0.9 + 0.2 * rng.next());
          this.chTau = Math.min(0.16, 0.5 / rate);
        }
      }
      if (this.chT < 1) {
        this.chT += 1 / sr;
        const e = this.chAmp * Math.min(1, this.chT / 0.004) * Math.exp(-this.chT / this.chTau);
        const nz = rng.bi();
        const c = (this.puff.process(nz) * 0.9 + this.body.process(nz) * 1.6) * e;
        L += c; R += c * 0.9;
      }
      // Rods: two clanks per turn of the wheels.
      if (rate > 0.2) {
        this.rodPh += rate / 2 / sr;
        if (this.rodPh >= 1) { this.rodPh -= 1; this.clankEnv = 0.15 * (0.3 + N); }
      }
      if (this.clankEnv > 1e-4) { const c = this.clank.process(rng.bi()) * this.clankEnv; L += c * 0.7; R += c; this.clankEnv *= Math.exp(-1 / (0.008 * sr)); }
      // Rail joints every 18 m, front and rear truck: clickety-clack.
      const v = rate / 4 * this.tire;                // m/s (wheel turns × circumference)
      if (v > 1) {
        this.railX += v / sr;
        if (this.railX >= 18 && this.nClicks < 15) { this.railX -= 18; this.clicks[this.nClicks++] = 0; this.clicks[this.nClicks++] = 2.5 / v; }
      }
      const ck = this.clicks;
      for (let k = 0; k < this.nClicks; k++) {
        ck[k] -= 1 / sr;
        if (ck[k] <= 0 && ck[k] > -0.015) {
          const tt = -ck[k];
          const c = (this.rail.process(rng.bi()) * 0.5 + Math.sin(TAU * 120 * tt) * 0.6) * Math.exp(-tt / 0.006) * 0.25;
          L += c; R += c;
        } else if (ck[k] <= -0.015) {   // done: close the gap
          for (let j = k; j < this.nClicks - 1; j++) ck[j] = ck[j + 1];
          this.nClicks--; k--;
        }
      }
      // Standing: the air pump thumps.
      if (rate <= 0.2) {
        this.pumpT += 1 / sr;
        if (this.pumpT > 1.1) { this.pumpT = 0; this.pumpEnv = 1; this.pumpPh = 0; }
      }
      if (this.pumpEnv > 1e-3) {
        this.pumpPh += 70 / sr;
        const p = this.pumpEnv * (Math.sin(this.pumpPh * TAU) * 0.35 + this.puff.process(rng.bi()) * 0.08);
        L += p; R += p;
        this.pumpEnv *= Math.exp(-1 / (0.08 * sr));
      }
      // Boiler hiss, louder working hard; and the blow-down on stopping.
      const hs = this.hiss.process(rng.bi()) * (0.015 + (rate > 0.2 ? 0.02 + 0.03 * load : 0));
      L += hs; R += hs * 0.8;
      if (this.blowEnv > 1e-3) {
        const b = this.blow.process(rng.bi()) * this.blowEnv * 0.9;
        L += b; R += b;
        this.blowEnv *= Math.exp(-1 / (1.1 * sr));
      }
      // Whistle: three chimes with breath, sliding up into pitch.
      if (this.horn > 1e-3) {
        this.hornT += 1 / sr;
        const bend = 1 - 0.04 * Math.exp(-this.hornT / 0.12);
        let w = 0;
        for (const c of this.chime) {
          c.ph += c.f * bend / sr; c.ph -= Math.floor(c.ph);
          w += Math.sin(c.ph * TAU) * 0.18 + c.bp.process(rng.bi()) * 0.6;
        }
        w *= this.horn;
        L += w; R += w;
      } else this.hornT = 0;
      outL[i] = this._out(L * 4, 1.4);
      outR[i] = this._out(R * 4, 1.4);
    }
  }
}

export const SPECIAL = { jet: Jet, pod: Pod, warp: Warp, steam: Steam };
