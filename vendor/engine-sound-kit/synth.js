// synth.js — procedural engine sound.
//
// Pure DSP with no DOM access, so the same code runs in the AudioWorklet, in a
// ScriptProcessor fallback on the main thread, and in Node for offline renders.
//
// Signal path, per sample:
//   crank angle → cylinder firings (the car's firing order) → an exhaust
//   pressure pulse plus combustion noise into that cylinder's bank, after its
//   header delay → turbine smoothing → X/H-pipe crossover → tailpipe
//   waveguides (delay line with a lossy, inverting reflection) panned to
//   stereo → body resonances → muffler low-pass → plus intake roar,
//   valvetrain, turbo whistle / blow-off, supercharger whine, gearbox whine
//   and afterfire pops → DC block → saturation.
//
// Driven by smoothed values from the drivetrain: rpm, load (0..1 combustion
// strength), overrun (0..1 closed-throttle coasting), cut (1 while the
// ignition is cut), starter (0..1) and shaft (gearbox output rpm).

import { SPECIAL } from './special.js';

const TAU = Math.PI * 2;
const BLOCK = 32;
const MAX_PULSES = 48;
const MAX_POPS = 16;
const MAX_BANGS = 16;   // up to 6 ringing + a burst of up to 10 (Tune allows 10)
const MAX_FLAMES = 4;
const SHAPE_N = 2048;
const SHAPE_END = 8;   // pulses are computed out to 8 pulse widths

function clamp(x, lo, hi) { return x < lo ? lo : x > hi ? hi : x; }
function lerp(a, b, t) { return a + (b - a) * t; }
function smoothstep(x) { x = clamp(x, 0, 1); return x * x * (3 - 2 * x); }
function coef(sr, seconds) { return 1 - Math.exp(-1 / (sr * Math.max(seconds, 1e-5))); }

// Pulse shape x·e^(1-x) (peaks at 1 when x = 1, long smooth tail) and the
// envelope of the noise burst riding on it, tabulated over 0..SHAPE_END.
const PULSE = new Float32Array(SHAPE_N + 1);
const BURST = new Float32Array(SHAPE_N + 1);
for (let i = 0; i <= SHAPE_N; i++) {
  const x = i / SHAPE_N * SHAPE_END;
  PULSE[i] = x * Math.exp(1 - x);
  BURST[i] = Math.exp(-x / 1.4) * Math.min(1, x * 4);
}

class Rng {
  constructor(seed) { this.s = (seed >>> 0) || 0x2545f491; }
  next() {
    let s = this.s;
    s ^= s << 13; s ^= s >>> 17; s ^= s << 5;
    this.s = s >>> 0;
    return this.s / 4294967296;
  }
  bi() { return this.next() * 2 - 1; }
}

// RBJ cookbook biquad, transposed direct form II.
class Biquad {
  constructor() { this.b0 = 1; this.b1 = 0; this.b2 = 0; this.a1 = 0; this.a2 = 0; this.z1 = 0; this.z2 = 0; }
  _set(b0, b1, b2, a0, a1, a2) {
    this.b0 = b0 / a0; this.b1 = b1 / a0; this.b2 = b2 / a0; this.a1 = a1 / a0; this.a2 = a2 / a0;
  }
  lowpass(sr, hz, q) {
    const w = TAU * clamp(hz, 10, sr * 0.45) / sr, c = Math.cos(w), al = Math.sin(w) / (2 * q);
    this._set((1 - c) / 2, 1 - c, (1 - c) / 2, 1 + al, -2 * c, 1 - al);
  }
  bandpass(sr, hz, q) {
    const w = TAU * clamp(hz, 10, sr * 0.45) / sr, c = Math.cos(w), al = Math.sin(w) / (2 * q);
    this._set(al, 0, -al, 1 + al, -2 * c, 1 - al);
  }
  lowshelf(sr, hz, db) {
    const A = Math.pow(10, db / 40);
    const w = TAU * clamp(hz, 10, sr * 0.45) / sr, c = Math.cos(w), al = Math.sin(w) / 2 * Math.SQRT2, r = 2 * Math.sqrt(A) * al;
    this._set(A * ((A + 1) - (A - 1) * c + r), 2 * A * ((A - 1) - (A + 1) * c), A * ((A + 1) - (A - 1) * c - r),
      (A + 1) + (A - 1) * c + r, -2 * ((A - 1) + (A + 1) * c), (A + 1) + (A - 1) * c - r);
  }
  peak(sr, hz, q, db) {
    const A = Math.pow(10, db / 40);
    const w = TAU * clamp(hz, 10, sr * 0.45) / sr, c = Math.cos(w), al = Math.sin(w) / (2 * q);
    this._set(1 + al * A, -2 * c, 1 - al * A, 1 + al / A, -2 * c, 1 - al / A);
  }
  process(x) {
    const y = this.b0 * x + this.z1;
    this.z1 = this.b1 * x - this.a1 * y + this.z2;
    this.z2 = this.b2 * x - this.a2 * y;
    return y;
  }
}

// Tailpipe: a delay line fed back through a one-pole low-pass (wall losses)
// and an inverting reflection (open pipe end). Resonates at odd multiples of
// 1 / (2 · delay).
class Pipe {
  constructor(sr) {
    this.buf = new Float32Array(Math.ceil(sr * 0.05) + 4);
    this.w = 0; this.lp = 0; this.d = 100; this.r = -0.5; this.k = 0.7;
  }
  set(sr, ms, reflect, damp) {
    this.d = clamp(ms * sr / 1000, 2, this.buf.length - 3);
    this.r = clamp(reflect, -0.97, 0.97);
    this.k = 1 - clamp(damp, 0, 0.95);
  }
  process(x) {
    const buf = this.buf, len = buf.length;
    let rp = this.w - this.d;
    if (rp < 0) rp += len;
    const i = rp | 0, f = rp - i, j = i + 1 === len ? 0 : i + 1;
    this.lp += this.k * (buf[i] + (buf[j] - buf[i]) * f - this.lp);
    const y = x + this.r * this.lp;
    buf[this.w] = y;
    this.w = this.w + 1 === len ? 0 : this.w + 1;
    return y;
  }
}

// The voice's reusable slots and small state records are classes whose
// number fields start as -0 (equal to 0 in every sum and comparison). V8
// stores a field that starts as a whole number differently from one holding
// fractions, and switching later leaves every new voice on the old layout:
// the first time the per-sample loop met one (a gunshot's crackMul of 1.9,
// say) it had to recompile, an audible stall. -0 is stored as a fraction from
// the start, so the layout never changes.
class Pulse { constructor() { this.on = false; this.wait = -0; this.age = -0; this.step = -0; this.amp = -0; this.nAmp = -0; this.bank = 0; } }
class Pop { constructor() { this.on = false; this.wait = -0; this.age = 0; this.amp = -0; this.pan = -0; this.hz = -0; this.body = -0; this.n1 = -0; this.n2 = -0; } }
class Bang { constructor() { this.wait = -0; this.t = 0; this.amp = -0; this.ph = -0; this.sub = -0; this.lp = -0; this.lp2 = -0; this.single = false; this.bp = -0; this.bp2 = -0; this.crackMul = -0; } }
class Flame { constructor() { this.wait = -0; this.t = 0; this.amp = -0; this.lp = -0; this.lp2 = -0; this.ph = -0; } }
const BODY = 0.18;   // load-equivalent of engine braking with body on (a light cruise is ~0.15–0.2)

class Params { constructor() { this.rpm = -0; this.load = -0; this.overrun = -0; this.cut = -0; this.starter = -0; this.shaft = -0; this.horn = -0; this.pullAway = -0; this.body = -0; } }
class DcBlock { constructor() { this.x = -0; this.y = -0; } }

export class EngineVoice {
  constructor(sr, car, seed) {
    this.sr = sr;
    this.rng = new Rng(seed || 12345);
    this.target = new Params();
    this.rpm = 0; this.load = 0; this.overrun = 0; this.cut = 0; this.starter = 0; this.shaft = 0;
    this.body = -0;   // engine braking keeps a light-cruise body (phone connected; see _fire)
    this.character = 0.5;
    this.crackleUser = 0.5;
    this.kRpm = coef(sr, 0.018);
    this.kLoad = coef(sr, 0.03);

    this.toNext = 0; this.next = 0; this.fireLag = 0;
    this.pulses = [];
    for (let i = 0; i < MAX_PULSES; i++) this.pulses.push(new Pulse());
    this.pulseTop = 0;   // pulses at this index and above are all off
    this.pops = [];
    for (let i = 0; i < MAX_POPS; i++) this.pops.push(new Pop());

    this.bankIn = new Float32Array(4);
    this.turbLp = new Float32Array(4);
    this.pipes = [];
    this.bodyL = []; this.bodyR = [];
    for (let i = 0; i < 6; i++) { this.bodyL.push(new Biquad()); this.bodyR.push(new Biquad()); }
    this.mufL = new Biquad(); this.mufR = new Biquad();
    this.bassL = new Biquad(); this.bassR = new Biquad();
    this.toneL = new Biquad(); this.toneR = new Biquad();
    this.thumpBp = new Biquad(); this.thumpBp2 = new Biquad();
    this.intakeBp = new Biquad();
    this.mechBp = new Biquad(); this.mechBp.bandpass(sr, 4200, 1.2);
    this.hissBp = new Biquad(); this.hissBp.bandpass(sr, 5200, 0.7);
    this.rushBp = new Biquad();
    this.bovBp = new Biquad();
    this.starterBp = new Biquad(); this.starterBp.bandpass(sr, 1800, 2);
    this.clickBp = new Biquad(); this.clickBp.bandpass(sr, 2600, 1.5);
    this.popLp = 0;

    this.firingEnv = 0;
    this.mechEnv = 0;
    this.boost = 0; this.turboSpool = 0; this.peakLoad = 0; this.peakBoost = 0; this.bovEnv = 0; this.bovT = 9; this.bovPhase = 0; this.bovLp = 0; this.bovArmed = false;
    this.starterRpm = 0; this.starterPh2 = 0; this.camPh = 0;
    this.mixE = 1; this.mixB = 1; this.mixP = 1;
    this.whistlePh = 0; this.whistlePh2 = 0; this.whistleU = 0; this.wPf = 0; this.wPs = 0; this.blowPh = 0; this.blowPh2 = 0; this.gearPh = 0; this.starterPh = 0;
    this.clickEnv = 0; this.clickT = 1; this.thumpPh = 0;
    // Backfires and flames live in fixed slots, made once and reused, so the
    // per-sample loop always sees the same objects (V8 keeps it compiled).
    // bangs[0..nBangs) are the live ones, oldest first; likewise flames.
    this.bangs = [];
    for (let i = 0; i < MAX_BANGS; i++) this.bangs.push(new Bang());
    this.nBangs = 0;
    this.airBp = new Biquad(); this.airBp.bandpass(sr, 3800, 0.7); this.airEnv = 0; this.airT = 9;
    this.alT = 9;      // anti-lag: time since the last big bang
    this.flames = [];  // exhaust flames
    for (let i = 0; i < MAX_FLAMES; i++) this.flames.push(new Flame());
    this.nFlames = 0;
    this.flameLp = 0;
    // Cartoon: kazoo phase and formants, the springy rev wobble, the honk.
    this.kazPh = 0; this.kazF1 = new Biquad(); this.kazF2 = new Biquad();
    this.spring = 0; this.springV = 0; this.lastRpm = 0; this.honkT = 9; this.honkPh = 0; this.honkBp = new Biquad();
    this.honkBp.bandpass(sr, 1100, 1.2);
    this.dcL = new DcBlock(); this.dcR = new DcBlock();

    this.configure(car);
  }

  // car: { engine: { idle, redline }, firing: [[deg, bank], ...], sound: {...} }
  // Safe to call while running: the crank keeps turning and nothing clicks.
  configure(car) {
    const sr = this.sr, s = car.sound;
    this.sound = s;
    // Not an engine at all (jet, steam, ...): its own generator does the work.
    const Special = SPECIAL[car.kind];
    if (Special) {
      if (!this.special || this.specialKind !== car.kind) this.special = new Special(sr, this.rng);
      this.specialKind = car.kind;
      this.special.configure(car);
    } else {
      this.special = null; this.specialKind = '';
    }
    this.idle = car.engine.idle;
    this.redline = car.engine.redline;
    this.firing = car.firing.map((f, i) => ({ deg: f[0], bank: f[1], idx: i })).sort((a, b) => a.deg - b.deg);
    const n = this.firing.length, cyc = car.cycle || 720;   // 360 for a two-stroke
    this.gaps = this.firing.map((f, i) => {
      const g = (this.firing[(i + 1) % n].deg - f.deg + cyc) % cyc;
      return g || cyc;
    });
    if (this.next >= n) this.next = 0;

    // Fixed cylinder-to-cylinder differences, the same every time for a car.
    const tr = new Rng(n * 7919 + 17);
    this.trim = this.firing.map(() => tr.bi() * s.trim);
    this.header = car.firing.map((_, i) => (s.header && s.header[i] ? s.header[i] * sr / 1000 : 0));

    this.nBanks = Math.max(1, ...car.firing.map(f => f[1] + 1));
    while (this.pipes.length < s.pipes.length) this.pipes.push(new Pipe(sr));
    // Pipes sharing a bank split its flow, so each gets an equal share.
    const perBank = new Array(this.nBanks).fill(0);
    for (const p of s.pipes) perBank[Math.min(p.bank || 0, this.nBanks - 1)]++;
    this.pipeCfg = s.pipes.map(p => {
      const bank = Math.min(p.bank || 0, this.nBanks - 1);
      const a = (clamp(p.pan, -1, 1) + 1) * Math.PI / 4;
      const w = Math.SQRT2 / perBank[bank];
      return { bank, gl: Math.cos(a) * w, gr: Math.sin(a) * w };
    });
    s.pipes.forEach((p, i) => this.pipes[i].set(sr, p.ms, s.reflect, s.damp));
    this.pipeNorm = 1 / Math.sqrt(this.nBanks);

    for (let i = 0; i < 6; i++) {
      const b = s.body[i];
      if (b) { this.bodyL[i].peak(sr, b.hz, b.q, b.db); this.bodyR[i].peak(sr, b.hz, b.q, b.db); }
      else { this.bodyL[i].peak(sr, 1000, 1, 0); this.bodyR[i].peak(sr, 1000, 1, 0); }
    }
    this.nBody = Math.min(6, s.body.length);

    // Thump: every firing rings a low resonance, the chest-felt boom of combustion.
    this.thumpBp.bandpass(sr, s.thump.hz, s.thump.q);
    this.thumpBp2.bandpass(sr, s.thump.hz, s.thump.q);
    this.bassL.lowshelf(sr, s.bass.hz, s.bass.db);
    this.bassR.lowshelf(sr, s.bass.hz, s.bass.db);
    // Optional final tone: rolls the top off the engine (not the turbo or
    // bangs laid on top), for builds that sound darker than the synth's
    // bare pulses.
    this.toneOn = s.toneHz > 0;
    if (this.toneOn) { this.toneL.lowpass(sr, s.toneHz, 0.7); this.toneR.lowpass(sr, s.toneHz, 0.7); }

    const t = s.turbo;
    this.turbK = 1 - Math.exp(-TAU * lerp(14000, 1400, clamp(t.damp * t.amt, 0, 1)) / sr);
    this.hissBp.bandpass(sr, t.hissHz || 5200, t.hissQ || 0.7);
    this.rushBp.bandpass(sr, 5200, 0.7);
    this.bovLpK = 1 - Math.exp(-TAU * 2500 / sr);
    this.wPfK = 1 - Math.exp(-TAU * 150 / sr); this.wPsK = 1 - Math.exp(-TAU * 3 / sr);

    // Turbo model: displacement drives exhaust flow; spool is shaft speed,
    // boost is compressor pressure. Both are first-order lags with different
    // time constants. Big displacement spools a given turbo faster; the
    // compressor map gates boost below a minimum shaft speed.
    this.turboDisp = car.engine.disp || 2.0;   // litres; default 2.0 if unset
    this.turboFlowRef = 3.0;                  // 3 L = reference flow (1.0)
  }

  setParams(p) {
    const t = this.target;
    if (p.rpm !== undefined) t.rpm = p.rpm;
    if (p.load !== undefined) t.load = p.load;
    if (p.overrun !== undefined) t.overrun = p.overrun;
    if (p.cut !== undefined) t.cut = p.cut;
    if (p.starter !== undefined) t.starter = p.starter;
    if (p.shaft !== undefined) t.shaft = p.shaft;
    if (p.horn !== undefined) t.horn = p.horn;
    if (p.pullAway !== undefined) t.pullAway = p.pullAway;
    if (p.body !== undefined) t.body = p.body;
  }

  // Mix (settings): engine, turbo/supercharger and pops/bangs levels, 1 = as voiced.
  setMix(engine, boost, pops) {
    if (engine !== undefined) this.mixE = clamp(engine, 0, 2);
    if (boost !== undefined) this.mixB = clamp(boost, 0, 2);
    if (pops !== undefined) this.mixP = clamp(pops, 0, 2);
  }

  setUser(character, crackle) {
    if (character !== undefined) this.character = clamp(character, 0, 1);
    if (crackle !== undefined) this.crackleUser = clamp(crackle, 0, 1);
  }

  click(amt) { this.clickEnv = Math.max(this.clickEnv, amt); this.clickT = 0; }

  // Drivetrain events: 'up' / 'down' / 'lift' / 'start' fire backfires;
  // 'stop' lets the air brakes hiss.
  event(type, amt) {
    if (this.special) { if (this.special.event) this.special.event(type, amt); return; }
    if (type === 'stop') {
      if (this.sound.airBrake > 0) { this.airEnv = this.sound.airBrake; this.airT = 0; }
      return;
    }
    // Letting off or shifting up with boost on: the turbo vents.
    // (A quick DCT upshift barely closes the throttle: bovShift 0 keeps it quiet.)
    if (type === 'lift') this._release(amt);
    else if (type === 'up' && this.sound.turbo.bovShift !== 0) this._release(amt * (this.sound.turbo.bovShift ?? 1));
    const g = this.sound.gunshot;
    if (g && g.amt > 0 && type === 'lift') { this.gunshot(amt); return; }
    if (g && g.flame > 0 && type === 'up') this.flame(0.5 * g.flame * amt, 25);
    this.bang(amt);
  }

  // Turbo pressure let go: a blow-off valve's "pssh" (or the flutter's
  // "stu-stu-stu" on tuner cars), then the whistle spools down quickly.
  // Volume follows the trapped boost pressure: a gentle lift after a small
  // pull is a soft pssh, a full lift on full boost is loud. With no boost
  // (startup, idle, coasting below the map floor) there is nothing to vent.
  _release(strength) {
    const t = this.sound.turbo;
    // One vent per boost build: after venting, the valve stays shut until the
    // turbo has been loaded up again (stops repeat psshs from one lift).
    if (t.amt <= 0 || t.bov === 'none' || this.boost < 0.08 || this.bovT < 0.25 || !this.bovArmed) return;
    this.bovArmed = false;
    this.bovEnv = Math.min(1.3, this.boost * 1.1 + 0.08) * t.bovAmt * (0.6 + 0.4 * clamp(strength, 0, 1));
    this.bovT = 0; this.bovPhase = 0;
    this.boost *= 0.3; this.turboSpool *= 0.4;
    // One lift, one vent: forget the peaks so the load-drop check below
    // doesn't fire a second pssh for the same lift.
    this.peakLoad = 0; this.peakBoost = 0;
  }

  // Lift-off gunshot: one huge, dry crack-and-boom, then (if the car spits
  // fire) a fireball that rolls out of the pipes a beat later.
  gunshot(strength) {
    const s = this.sound, g = s.gunshot;
    if (this.nBangs > 6) return;
    const amt = g.amt * this.crackleUser * 2 * (0.6 + 0.4 * strength);
    if (amt < 0.02) return;
    this._addBang(8 * this.sr / 1000, amt, true, 1.9);
    if (g.flame > 0) this.flame(g.flame * strength * this.crackleUser * 2, 14);
  }

  // Flame: a soft low "whoomp" of burning fuel, then a few hard crackles.
  flame(amp, delayMs) {
    if (amp < 0.02 || this.nFlames > 3) return;
    const f = this.flames[this.nFlames++];
    f.wait = delayMs * this.sr / 1000; f.t = 0; f.amp = amp; f.lp = 0; f.lp2 = 0; f.ph = 0;
  }

  // A single loud backfire (on a hard upshift or a sharp lift-off): a crack,
  // then a deep boom whose pitch drops, then a low rumble.
  //
  // Some cars fire a burst ("ra-ta-ta"): several bangs 30-50 ms apart, each a
  // sharp crack with its own short sub-bass boom.
  bang(strength) {
    const s = this.sound, rng = this.rng;
    if (s.toon > 0) { if (this.honkT > 0.6) { this.honkT = 0; this.honkPh = 0; } return; }   // a-wooga
    const amt = s.bang * this.crackleUser * 2 * strength;
    if (amt < 0.02 || this.nBangs > 6) return;
    const [lo, hi] = s.bangBurst;
    const count = Math.max(1, Math.round(lo + rng.next() * (hi - lo) * Math.min(1, strength + 0.2)));
    let wait = (15 + rng.next() * 30) * this.sr / 1000;
    for (let i = 0; i < count; i++) {
      const a = amt * (i === 0 ? 1 : 0.55 + 0.45 * rng.next());
      this._addBang(wait, a, count === 1, 1);
      wait += s.bangGapMs * (0.7 + 0.6 * rng.next()) * this.sr / 1000;
    }
  }

  _addBang(wait, amp, single, crackMul) {
    const g = this.bangs[this.nBangs++];
    g.wait = wait; g.t = 0; g.amp = amp; g.ph = 0; g.sub = 0; g.lp = 0; g.lp2 = 0;
    g.single = single; g.bp = 0; g.bp2 = 0; g.crackMul = crackMul;
  }

  _fire(f) {
    const s = this.sound, rng = this.rng, rpm = this.rpm;
    const rpmN = clamp((rpm - this.idle) / (this.redline - this.idle), 0, 1);
    const ch = this.character;
    const load = this.load, cut = this.cut;

    // Pumping pulse (the exhaust valve opens even without combustion) plus
    // combustion, which grows with load and vanishes while the spark is cut.
    const ov = this.overrun * (1 - load);
    // body (0..1): engine braking at the level of a light cruise instead of
    // dropping well below it (it keeps its darker drone and crackle). Only
    // set while a phone feeds the Tesla; 0 is the sound as it always was.
    const lb = Math.max(load, BODY * this.body * this.overrun);
    let amp = 0.13 * (1 - 0.3 * ov) + (1 - cut) * (0.12 + 0.88 * lb);
    // Idle roll: how much of the cylinder-to-cylinder unevenness (trim,
    // roughness, lope) survives near idle; fades back to 1 by +1,500 rpm.
    const idleN = 1 - smoothstep((rpm - this.idle) / 1500);
    const roll = 1 + (s.idleRoll - 1) * idleN;
    amp *= (0.55 + 0.45 * rpmN) * (1 + this.trim[f.idx] * roll) * (1 + s.rough * roll * 2 * rng.bi());
    // Lumpy cam: uneven combustion near idle, fading out by +1,500 rpm.
    const lope = s.lope * roll * (0.6 + 0.8 * ch) * idleN;
    if (lope > 0) {
      amp *= 1 + lope * 1.6 * rng.bi();
      if (rng.next() < lope * 0.25 * (0.3 + this.crackleUser)) amp *= 0.3;
    }
    // Overrun: a steady, softer drone. overrunRough adds a little unevenness.
    if (ov > 0.05 && s.overrunRough > 0) {
      amp *= 1 + ov * s.overrunRough * 1.4 * rng.bi();
      if (rng.next() < ov * s.overrunRough * 0.3) amp *= 0.25;
    }
    amp = Math.max(amp, 0.02);
    let nAmp = s.noise * (0.6 + 0.8 * ch) * (0.2 * (1 - 0.6 * ov) + 0.8 * lb * (1 - cut)) * amp;
    let wMs = lerp(s.pulse.idleMs, s.pulse.topMs, Math.pow(rpmN, 0.7)) * (1 + s.overrunDark * 0.8 * ov);

    // Jake brake: on overrun the exhaust valves open at the top of each
    // compression stroke and the squeezed air blasts out: a hard bark per
    // cylinder, the machine-gun "brap".
    const jk = s.jake && s.jake.amt > 0 && rpm > this.idle * 1.3 ? s.jake.amt * smoothstep((ov - 0.2) / 0.5) : 0;
    if (jk > 0) {
      amp = Math.max(amp, jk * (0.55 + 0.45 * rpmN));
      nAmp = Math.max(nAmp, jk * 1.3 * amp);
      wMs *= 1 - 0.45 * jk / s.jake.amt;
    }

    // Anti-lag: off throttle, fuel and spark are kept going late so it burns
    // in the exhaust and keeps the turbo lit: pops on most firings and
    // regular big bangs.
    if (s.antilag > 0 && ov > 0.3 && rpm > this.idle * 1.4) {
      const al = s.antilag * this.crackleUser * 2;
      if (rng.next() < al * 0.22) this._pop(s.popAmt * (0.7 + 0.6 * rng.next()));
      if (this.alT > 0.18 + rng.next() * 0.5 && rng.next() < al * 0.05) { this.bang(0.35 + 0.4 * rng.next()); this.alT = 0; }
    }
    const wSamp = Math.max(wMs * this.sr / 1000, 1.5);
    const step = SHAPE_N / SHAPE_END / wSamp;   // table index advance per sample

    const P = this.pulses;
    for (let i = 0; i < MAX_PULSES; i++) {
      const p = P[i];
      if (p.on) continue;
      p.on = true; p.bank = f.bank; p.amp = amp; p.nAmp = nAmp; p.step = step;
      p.wait = this.header[f.idx]; p.age = this.fireLag * step;
      if (i >= this.pulseTop) this.pulseTop = i + 1;
      break;
    }
    this.mechEnv = 1;

    // Afterfire: unburnt fuel lighting off in the hot exhaust on overrun or
    // during an ignition cut.
    const cr = s.crackle * this.crackleUser * 2;
    if (cr > 0 && rpm > this.idle * 1.35) {
      const chance = cr * (0.5 + ch) * (cut > 0.5 ? 0.045 : 0.012 * this.overrun);
      if (rng.next() < chance) this._pop(s.popAmt * (0.5 + 0.9 * rng.next() * rng.next()));
    }
  }

  _pop(amp) {
    const rng = this.rng, pops = this.pops;
    for (let i = 0; i < MAX_POPS; i++) {
      const q = pops[i];
      if (q.on) continue;
      q.on = true; q.age = 0;
      q.wait = (3 + rng.next() * 30) * this.sr / 1000;
      q.amp = amp;
      q.hz = 50 + rng.next() * 35;
      q.body = 250 + rng.next() * 350; q.n1 = 0; q.n2 = 0;
      q.pan = this.pipeCfg.length ? (rng.next() < 0.5 ? -0.5 : 0.5) : 0;
      break;
    }
  }

  _block() {
    const s = this.sound, sr = this.sr, dt = BLOCK / sr;
    const rpm = this.rpm, load = this.load, ch = this.character;
    const rpmN = clamp((rpm - this.idle) / (this.redline - this.idle), 0, 1);

    // Muffler opens with load, revs and Character.
    const ov = this.overrun * (1 - load), lb = Math.max(load, BODY * this.body * this.overrun);
    const open = clamp(0.1 + 0.6 * lb + 0.15 * rpmN + 0.35 * (ch - 0.5) - 0.35 * s.overrunDark * ov * (1 - 0.5 * this.body), 0, 1);
    const fc = s.muffler.closedHz * Math.pow(s.muffler.openHz / s.muffler.closedHz, open);
    this.mufL.lowpass(sr, fc, 0.6);
    this.mufR.lowpass(sr, fc, 0.6);
    this.intakeBp.bandpass(sr, s.intake.hz * (0.75 + 0.5 * rpmN), s.intake.q);

    // Turbo: a shaft with a turbine and a compressor on it. Exhaust gas spins
    // the turbine (spool); the compressor makes boost from shaft speed, gated
    // by the compressor map. Two first-order lags: the shaft's inertia, then
    // the compressor filling the intake.
    this.alT += dt; this.airT += dt; this.honkT += dt;
    const t = s.turbo;
    if (t.amt > 0) {
      // Exhaust energy spins the turbine. Displacement sets how FAST the
      // turbo spools (more gas = less lag), not how MUCH boost it can make:
      // a small engine revs harder to reach the same boost, just later. So
      // the boost ceiling is displacement-independent; only the spool rate
      // and the low-end flow (lugging) scale with engine size.
      const flowRef = this.turboDisp / this.turboFlowRef;   // 1.0 at 3 L
      // Ceiling the engine can drive the turbo to (any real turbo reaches
      // full boost); a little flow at low rpm in a tall gear from big
      // displacement is what lights the turbo when lugging.
      let energy = load * smoothstep(rpm / (t.spoolRpm * 0.6)) * (1 - this.cut * 0.5);
      if (s.antilag > 0 && rpm > this.idle * 1.4) energy = Math.max(energy, s.antilag * 0.8 * this.overrun);
      const flow = Math.min(1, energy * (0.7 + 0.6 * flowRef));

      // The shaft's inertia chases exhaust energy. A given turbo on a bigger
      // engine spools sooner (more gas for the same size); spoolS is the
      // per-car turbo-size knob, scaled by displacement so a 15 L diesel
      // lights from idle and a 1.6 L F1 car is lazy until it's screaming.
      const spoolTau = Math.max(0.05, t.spoolS * clamp(1.6 - 0.6 * flowRef, 0.4, 1.6));
      this.turboSpool += (flow - this.turboSpool) * clamp(dt / spoolTau, 0, 1);

      // The compressor map: no boost below a minimum shaft speed no matter
      // the exhaust (why revving in neutral barely builds), then pressure
      // rises with shaft speed, with its own short fill lag.
      const mapFloor = 0.18;
      const boostTgt = smoothstep((this.turboSpool - mapFloor) / (1 - mapFloor));
      this.boost += (boostTgt - this.boost) * clamp(dt / 0.22, 0, 1);

      // Lift-off: the throttle snaps shut with boost trapped ahead of it.
      // The BOV vents it — louder with more trapped pressure. Fires on the
      // 'up'/'lift' events (see _release) and on a sharp load drop here.
      this.peakLoad = Math.max(load, this.peakLoad - dt * 0.3);
      this.peakBoost = Math.max(this.boost, (this.peakBoost || 0) - dt * 0.3);
      this.bovT += dt;
      if (load > 0.4 && this.boost > 0.3) this.bovArmed = true;
      if (t.bov !== 'none' && this.peakBoost > 0.1 && this.peakLoad > 0.2 && this.peakLoad - load > 0.15 && load < Math.max(0.12, this.peakLoad * 0.75) && this.bovT > 1.2) {
        this._release(0.7);   // strength is neutral; volume comes from trapped boost
        this.peakLoad = load;
      }
      if (this.bovEnv > 1e-4) {
        const e = this.bovEnv / Math.max(t.bovAmt, 1e-3);
        if (t.bov === 'flutter') this.bovBp.bandpass(sr, 650 + 700 * e, 1.6);
        else if (t.bovHz) this.bovBp.bandpass(sr, t.bovHz[0] + (t.bovHz[1] - t.bovHz[0]) * Math.min(e, 1), t.bovQ || 1.1);
        else this.bovBp.bandpass(sr, 1700 + 2300 * e, 1.1);
        this.bovEnv *= Math.exp(-dt / (t.bov === 'flutter' ? 0.55 : (t.bovDecay || 0.4)));
      }
    } else {
      this.boost = 0; this.turboSpool = 0; this.bovEnv = 0;
    }
  }

  render(outL, outR, n) {
    if (this.special) { this.special.render(outL, outR, n, this); return; }
    const s = this.sound, sr = this.sr, rng = this.rng;
    const P = this.pulses, pops = this.pops, pipes = this.pipes, pc = this.pipeCfg;
    const nPipes = pc.length, nBanks = this.nBanks, bankIn = this.bankIn, turbLp = this.turbLp;
    const tg = this.target;
    const cross = clamp(s.cross || 0, 0, 1);
    const t = s.turbo, bl = s.blower, gw = s.gearWhine;
    const drive = s.drive * (0.85 + 0.3 * this.character);
    const satNorm = s.gain / Math.tanh(drive);
    const mixE = this.mixE, mixB = this.mixB, mixP = this.mixP;
    const kPop = 1 - Math.exp(-TAU * 5500 / sr);
    const mechK = Math.exp(-1 / (0.0012 * sr));
    const clickK = Math.exp(-1 / (0.004 * sr));
    // The noise generator, written out (Rng.bi / Rng.next, same arithmetic)
    // with its state in a local; handed back to rng around _fire, which uses
    // it too, and at the end.
    let rs = rng.s;

    for (let i = 0; i < n; i++) {
      if ((i % BLOCK) === 0) {
        this.overrun += (tg.overrun - this.overrun) * 0.08;
        this.body += (tg.body - this.body) * 0.08;
        this.starter += (tg.starter - this.starter) * 0.05;
        this.shaft += (tg.shaft - this.shaft) * 0.05;
        this.cut = tg.cut;
        this._block();
      }
      this.rpm += (tg.rpm - this.rpm) * this.kRpm;
      this.load += (tg.load - this.load) * this.kLoad;
      const rpm = this.rpm, load = this.load;
      const rpmN = clamp((rpm - this.idle) / (this.redline - this.idle), 0, 1);
      // With body on (see _fire), engine braking keeps a light-cruise level:
      // lb stands in for load in the layers that would otherwise fall away,
      // ovq for how far they back off.
      const lb = Math.max(load, BODY * this.body * this.overrun), ovq = this.overrun * (1 - load) * (1 - 0.7 * this.body);
      // Coasting: the bright layers (intake, valvetrain) back off with the exhaust.
      const quiet = 1 - 0.8 * s.overrunDark * ovq;

      // Crank: fire every cylinder whose angle we passed this sample.
      const dDeg = rpm * 6 / sr;
      if (dDeg > 0) {
        this.toNext -= dDeg;
        let guard = 0;
        while (this.toNext <= 0 && guard++ < 8) {
          this.fireLag = -this.toNext / dDeg;
          rng.s = rs;
          this._fire(this.firing[this.next]);
          rs = rng.s;
          this.toNext += this.gaps[this.next];
          this.next = (this.next + 1) % this.firing.length;
        }
      }

      // Exhaust pulses into their banks. Only slots below pulseTop can be on.
      for (let b = 0; b < nBanks; b++) bankIn[b] = 0;
      let env = 0, thumpIn = 0, hiOn = 0;
      for (let k = 0; k < this.pulseTop; k++) {
        const p = P[k];
        if (!p.on) continue;
        hiOn = k + 1;
        if (p.wait > 0) { p.wait--; continue; }
        const idx = p.age | 0;
        if (idx >= SHAPE_N) { p.on = false; continue; }
        const sh = PULSE[idx];
        bankIn[p.bank] += p.amp * sh + p.nAmp * BURST[idx] * (rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1);
        env += sh;
        thumpIn += p.amp * sh;
        p.age += p.step;
      }
      this.pulseTop = hiOn;
      this.firingEnv = env;

      // Turbine smoothing, then the crossover pipe.
      let mean = 0;
      for (let b = 0; b < nBanks; b++) {
        turbLp[b] += this.turbK * (bankIn[b] - turbLp[b]);
        mean += turbLp[b];
      }
      mean /= nBanks;

      // The per-sample filters and pipes below are Pipe.process and
      // Biquad.process written out by hand (same arithmetic). V8 only inlines
      // so much into one function; past that, every call boxes its numbers
      // and the garbage makes the audio thread stall now and then.
      let L = 0, R = 0;
      for (let k = 0; k < nPipes; k++) {
        const c = pc[k];
        const x = (1 - cross) * turbLp[c.bank] + cross * mean;
        const pp = pipes[k], buf = pp.buf, len = buf.length;
        let rp = pp.w - pp.d;
        if (rp < 0) rp += len;
        const ri = rp | 0, rf = rp - ri, rj = ri + 1 === len ? 0 : ri + 1;
        pp.lp += pp.k * (buf[ri] + (buf[rj] - buf[ri]) * rf - pp.lp);
        const y = x + pp.r * pp.lp;
        buf[pp.w] = y;
        pp.w = pp.w + 1 === len ? 0 : pp.w + 1;
        L += y * c.gl; R += y * c.gr;
      }
      L *= this.pipeNorm; R *= this.pipeNorm;

      for (let k = 0; k < this.nBody; k++) {
        const fl = this.bodyL[k], fr = this.bodyR[k];
        let y = fl.b0 * L + fl.z1; fl.z1 = fl.b1 * L - fl.a1 * y + fl.z2; fl.z2 = fl.b2 * L - fl.a2 * y; L = y;
        y = fr.b0 * R + fr.z1; fr.z1 = fr.b1 * R - fr.a1 * y + fr.z2; fr.z2 = fr.b2 * R - fr.a2 * y; R = y;
      }
      for (let k = 0; k < 2; k++) {   // muffler, then the bass shelf
        const fl = k ? this.bassL : this.mufL, fr = k ? this.bassR : this.mufR;
        let y = fl.b0 * L + fl.z1; fl.z1 = fl.b1 * L - fl.a1 * y + fl.z2; fl.z2 = fl.b2 * L - fl.a2 * y; L = y;
        y = fr.b0 * R + fr.z1; fr.z1 = fr.b1 * R - fr.a1 * y + fr.z2; fr.z2 = fr.b2 * R - fr.a2 * y; R = y;
      }
      if (s.thump.amt > 0) {
        let th = thumpIn;
        for (let k = 0; k < 2; k++) {
          const f = k ? this.thumpBp2 : this.thumpBp;
          const y = f.b0 * th + f.z1; f.z1 = f.b1 * th - f.a1 * y + f.z2; f.z2 = f.b2 * th - f.a2 * y; th = y;
        }
        // Gentle pull-away: the boom follows idle-level load, a bit quieter.
        const pa = tg.pullAway || 0;
        const tl = lb - Math.max(0, lb - 0.15) * pa;
        const k = s.thump.amt * 6 * (0.45 + 0.55 * tl) * (1 - 0.4 * ovq) * (1 - 0.3 * pa);
        L += th * k; R += th * k;
        // Sub: cylinder-to-cylinder differences put energy at half and full
        // crank speed (25 and 50 Hz at 3,000 rpm), pulsing with the firings.
        this.camPh += rpm / 120 / sr; if (this.camPh > 1) this.camPh -= 1;
        const a = this.camPh * TAU;
        // Near idle these slow beats are the "roll"; idleRoll tames them.
        const roll = 1 + (s.idleRoll - 1) * (1 - smoothstep((rpm - this.idle) / 1500));
        const sub = (0.7 * roll * Math.sin(a) + (1.25 - 0.25 * roll) * Math.sin(2 * a + 0.6)) * (0.55 + 0.45 * Math.min(env, 1.5));
        const ks = s.thump.amt * s.thump.sub * 0.45 * (0.35 + 0.65 * tl) * (1 - 0.5 * ovq) * (1 - 0.3 * pa)
          * (1 - 0.5 * smoothstep((rpm - 5000) / 5000));
        L += sub * ks; R += sub * ks;
      }

      // Everything below bypasses the muffler. C goes through the saturator
      // with the engine; T is laid on top of it, clean.
      let C = 0, T = 0;

      if (s.intake.amt > 0) {
        const f = this.intakeBp, x = (rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1);
        const nz = f.b0 * x + f.z1; f.z1 = f.b1 * x - f.a1 * nz + f.z2; f.z2 = f.b2 * x - f.a2 * nz;
        C += nz * s.intake.amt * quiet * (0.12 + 0.88 * lb) * (0.3 + 0.7 * rpmN) * (0.4 + Math.min(env, 2));
      }
      if (s.mech > 0) {
        this.mechEnv *= mechK;
        const f = this.mechBp, x = (rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1);
        const y = f.b0 * x + f.z1; f.z1 = f.b1 * x - f.a1 * y + f.z2; f.z2 = f.b2 * x - f.a2 * y;
        C += y * s.mech * quiet * this.mechEnv * (0.5 + 0.5 * rpmN);
      }
      if (t.amt > 0) {
        const b = this.boost;
        if (b > 1e-3 || t.whistleIdle > 0) {
          let wHz, wAmp;
          if (t.whistleFade) {
            // Compressor blade-pass: pitch follows the shaft, starting low and
            // sweeping up faster and faster as it spools; loudest low in the
            // sweep, thinning out as it climbs out of earshot at full spool.
            // Off throttle the shaft coasts: the pitch sags slowly as the
            // whistle dies away with the boost.
            const sp = this.turboSpool;
            const u = this.whistleU = Math.max(sp * sp * sp * sp, this.whistleU - 0.3 / sr);
            wHz = t.whistleHz[0] * Math.pow(t.whistleHz[1] / t.whistleHz[0], u);
            wAmp = smoothstep(sp / 0.5) * smoothstep((b - 0.45) / 0.45) * (1 - t.whistleFade * u);
            // A faint whine even at idle (and as the shaft spins down after a lift).
            if (t.whistleIdle > 0) wAmp = Math.max(wAmp, t.whistleIdle * (1 - t.whistleFade * u));
            // Each exhaust pulse kicks the turbine: near idle the whine pulses
            // with the firing (the beat smooths out as the revs rise).
            if (t.whistlePulse > 0) {
              this.wPf += this.wPfK * (env - this.wPf);
              this.wPs += this.wPsK * (this.wPf - this.wPs);
              const d = t.whistlePulse * (1 - smoothstep((rpm - this.idle) / 2500));
              wAmp *= 1 - d + d * Math.min(2.5, this.wPf / (this.wPs + 1e-6));
            }
          } else { wHz = lerp(t.whistleHz[0], t.whistleHz[1], b); wAmp = b * b; }
          this.whistlePh += wHz / sr;
          if (this.whistlePh > 1) this.whistlePh -= 1;
          const ph = this.whistlePh * TAU;
          let wv = Math.sin(ph) + 0.25 * Math.sin(2 * ph);
          // Twin turbos never spin quite together: a second whine a few percent
          // higher beats against the first (a rough, buzzy whistle).
          if (t.whistleTwin > 0) {
            this.whistlePh2 += wHz * t.whistleTwin / sr;
            if (this.whistlePh2 > 1) this.whistlePh2 -= 1;
            wv += 0.6 * Math.sin(this.whistlePh2 * TAU);
          }
          T += 0.6 * t.whistle * t.amt * wAmp * wv * mixB;
          const f = this.hissBp, x = (rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1);
          const y = f.b0 * x + f.z1; f.z1 = f.b1 * x - f.a1 * y + f.z2; f.z2 = f.b2 * x - f.a2 * y;
          // With a tone filter the turbo's rush sits under it with the engine.
          if (this.toneOn) C += 0.7 * y * t.hiss * t.amt * b * mixB; else T += 0.7 * y * t.hiss * t.amt * b * mixB;
          // Rush: on hard, high-boost pulls, bright air roaring through the
          // compressor on top of everything (clean, past the tone filter).
          if (t.rush > 0) {
            const k = smoothstep((b - 0.5) / 0.4) * smoothstep((load - 0.4) / 0.4);
            if (k > 0) {
              const g = this.rushBp, xr = (rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1);
              const yr = g.b0 * xr + g.z1; g.z1 = g.b1 * xr - g.a1 * yr + g.z2; g.z2 = g.b2 * xr - g.a2 * yr;
              T += yr * t.rush * t.amt * k * mixB;
            }
          }
        }
        if (this.bovEnv > 1e-4) {
          let e = this.bovEnv * Math.min(1, this.bovT / (t.bovAttack || 1 / 120));
          if (t.bov === 'flutter') {
            const r = this.bovEnv / Math.max(t.bovAmt, 1e-3);
            this.bovPhase += (11 + 17 * r) / sr;
            const m = 0.5 + 0.5 * Math.sin(this.bovPhase * TAU);
            e *= m * m * m * 1.8;
          }
          let x = (rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1);
          // A tuned valve (bovHz) sounds fuller: darker air, more body.
          if (t.bovHz) { this.bovLp += this.bovLpK * (x - this.bovLp); x = this.bovLp * 2; }
          const f = this.bovBp;
          const y = f.b0 * x + f.z1; f.z1 = f.b1 * x - f.a1 * y + f.z2; f.z2 = f.b2 * x - f.a2 * y;
          T += y * e * (t.bov === 'flutter' ? 3.2 : 1.8) * mixB;
        }
      }
      if (bl.amt > 0) {
        const f = rpm / 60 * bl.ratio;
        this.blowPh += f / sr; if (this.blowPh > 1) this.blowPh -= 1;
        this.blowPh2 += f * 1.0031 / sr; if (this.blowPh2 > 1) this.blowPh2 -= 1;
        const a = this.blowPh * TAU, a2 = this.blowPh2 * TAU;
        const tone = Math.sin(a) + 0.85 * Math.sin(2 * a) + 0.4 * Math.sin(3 * a) + 0.25 * Math.sin(4 * a)
          + 0.6 * (Math.sin(a2) + 0.7 * Math.sin(2 * a2));
        // Belt-driven: always spinning, louder under boost, quieter on overrun.
        T += tone * bl.amt * (0.25 * quiet + 0.75 * smoothstep(load * 1.5)) * (0.25 + 0.75 * rpmN) * 0.5 * mixB;
      }
      if (gw.amt > 0 && this.shaft > 50) {
        this.gearPh += this.shaft / 60 * gw.ratio / sr; if (this.gearPh > 1) this.gearPh -= 1;
        T += Math.sin(this.gearPh * TAU) * gw.amt * clamp(this.shaft / 2500, 0, 1) * (0.5 + 0.5 * load);
      }
      if (this.starter > 1e-3 || this.starterRpm > 5) {
        // The motor follows the crank while engaged (slowing on each compression
        // stroke), then spins down on its own once released.
        if (this.starter > 0.5) this.starterRpm += (rpm - this.starterRpm) * 0.01;
        else this.starterRpm *= 0.9996;
        const st = s.starter, motor = this.starterRpm * 13 / 60 * st.pitch;   // armature rev/s
        // A real starter is a rasp, not a tone. Two rich, rough sources: the
        // pinion gear mesh (teeth grinding the ring gear, a sawtooth) and the
        // commutator/brush whine (higher, square-ish). Grit swells with each
        // tooth for a mechanical "crunch"; the compression wobble already
        // loads the motor down through rpm.
        this.starterPh += motor * 9 / sr; if (this.starterPh > 2) this.starterPh -= 2;   // pinion mesh (≈9 teeth)
        this.starterPh2 += motor * 24 / sr; if (this.starterPh2 > 1) this.starterPh2 -= 1; // commutator segments
        const saw = this.starterPh - 1;                       // -1..1 mesh grind
        const a2 = this.starterPh2 * TAU;
        const lev = st.amt * Math.max(this.starter, this.starterRpm / 250 * 0.5);
        const comm = Math.sign(Math.sin(a2)) * 0.6 + 0.4 * Math.sin(a2 * 2);
        const f = this.starterBp, x = (rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1);
        const y = f.b0 * x + f.z1; f.z1 = f.b1 * x - f.a1 * y + f.z2; f.z2 = f.b2 * x - f.a2 * y;
        const grit = y * (0.4 + 0.6 * (saw * 0.5 + 0.5));
        C += lev * (saw * 0.04 + comm * 0.02 + grit * 0.03);
      }
      if (this.clickEnv > 1e-4) {
        this.clickT += 1 / sr;
        this.thumpPh += 140 / sr;
        const f = this.clickBp, x = (rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1);
        const y = f.b0 * x + f.z1; f.z1 = f.b1 * x - f.a1 * y + f.z2; f.z2 = f.b2 * x - f.a2 * y;
        C += this.clickEnv * (y * 0.5 + Math.sin(this.thumpPh * TAU) * 0.3 * Math.exp(-this.clickT / 0.02));
        this.clickEnv *= clickK;
      }

      // Air brakes: a sharp "pssht" with a long hiss tail.
      if (this.airEnv > 1e-4) {
        const e = this.airEnv * (Math.min(1, this.airT * 60) * (0.35 + 0.65 * Math.exp(-this.airT / 0.12)));
        T += this.airBp.process((rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1)) * e * 0.5;
        this.airEnv *= Math.exp(-1 / (0.9 * sr));
      }
      // Cartoon: a kazoo riding on the engine, its pitch on a spring that
      // overshoots every rev change; and the a-wooga horn.
      if (s.toon > 0) {
        const dr = (rpm - this.lastRpm) * sr; this.lastRpm = rpm;     // rpm per second
        const w = TAU * 5.5;
        this.springV += (-w * w * this.spring - 2 * 0.12 * w * this.springV + dr * 0.004) / sr;
        this.spring += this.springV / sr;
        const f = (70 + rpm * 0.045) * Math.exp(0.35 * clamp(this.spring, -2, 2));
        this.kazPh += f / sr; if (this.kazPh > 1) this.kazPh -= 1;
        if ((i & 31) === 0) {
          this.kazF1.bandpass(sr, lerp(450, 750, load), 4);
          this.kazF2.bandpass(sr, lerp(1000, 1400, load), 5);
        }
        const saw = 2 * this.kazPh - 1 + 0.15 * (rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1);
        const kz = this.kazF1.process(saw) * 1.2 + this.kazF2.process(saw) * 0.8;
        C += kz * s.toon * 0.18 * (0.35 + 0.65 * load) * (0.5 + 0.5 * Math.min(env, 1.5));
        if (this.honkT < 0.55) {
          const ht = this.honkT, hf = ht < 0.25 ? lerp(260, 420, ht / 0.25) : lerp(420, 300, (ht - 0.25) / 0.3);
          this.honkPh += hf / sr; if (this.honkPh > 1) this.honkPh -= 1;
          const sq = this.honkPh < 0.5 ? 1 : -1;
          T += this.honkBp.process(sq) * s.toon * s.bang * 0.5 * Math.min(1, ht * 40) * Math.min(1, (0.55 - ht) * 12);
        }
      }

      // Afterfire pops: a sharp crack plus a short low thump.
      let popL = 0, popR = 0;
      for (let k = 0; k < MAX_POPS; k++) {
        const q = pops[k];
        if (!q.on) continue;
        if (q.wait > 0) { q.wait--; continue; }
        const tt = q.age / sr;
        if (tt > 0.09) { q.on = false; continue; }
        q.n1 += 0.2 * ((rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1) - q.n1); q.n2 += 0.2 * (q.n1 - q.n2);
        const v = q.amp * (q.n2 * 4 * Math.exp(-tt / 0.005)
          + 0.8 * Math.sin(TAU * q.body * tt) * Math.exp(-tt / 0.01)
          + 1.3 * Math.sin(TAU * q.hz * tt) * Math.exp(-tt / 0.03));
        popL += v * (0.5 - q.pan * 0.5); popR += v * (0.5 + q.pan * 0.5);
        q.age++;
      }
      this.popLp += kPop * ((popL + popR) - this.popLp);
      const pl = (this.popLp * 0.5 + (popL - popR) * 0.2) * mixP;
      const pr = (this.popLp * 0.5 - (popL - popR) * 0.2) * mixP;

      L += C + pl; R += C + pr;

      // DC block, then saturate.
      let y = L - this.dcL.x + 0.9995 * this.dcL.y; this.dcL.x = L; this.dcL.y = y; L = y;
      y = R - this.dcR.x + 0.9995 * this.dcR.y; this.dcR.x = R; this.dcR.y = y; R = y;
      let B = 0;
      const bangs = this.bangs, flames = this.flames;
      for (let k = 0; k < this.nBangs; k++) {
        const g = bangs[k];
        if (g.wait > 0) { g.wait--; continue; }
        const tt = g.t / sr;
        // Boom: starts near 70 Hz and sweeps down to about 22 Hz, with a 30 Hz
        // sub underneath and a low rumble; only a small crack on top.
        // A lone bang rings longer; bangs in a burst are short and punchy.
        const len = g.single ? 1 : 0.4;
        g.ph += (22 + 48 * Math.exp(-tt / 0.04)) / sr;
        g.sub += 30 / sr;
        g.lp += 0.004 * ((rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1) - g.lp);
        g.lp2 += 0.004 * (g.lp - g.lp2);
        // Air: one quick puff (darkened noise, gone in about 2 ms) and a single
        // 1 ms push-pull pressure wave, the way a muzzle blast hits. No fizz.
        // Pop, not click: a rounded 2 ms pressure push, a dark puff of air and
        // a short hollow tone around 320 Hz, then the boom carries on.
        g.bp += 0.18 * ((rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1) - g.bp); g.bp2 += 0.18 * (g.bp - g.bp2);
        const push = tt < 0.002 ? Math.sin(Math.PI * tt / 0.002) : 0;
        const cm = g.crackMul || 1;
        const crack = (g.bp2 * 6 * Math.exp(-tt / 0.003) + push * 1.3
          + Math.sin(TAU * 320 * tt) * Math.exp(-tt / 0.008) * 0.9) * cm;
        const att = Math.min(1, tt / 0.002);
        B += g.amp * (crack * s.bangCrack * 1.6
          + att * Math.sin(g.ph * TAU) * Math.exp(-tt / (0.16 * len)) * 2.2
          + att * Math.sin(g.sub * TAU) * Math.exp(-tt / (0.22 * len)) * 1.4
          + g.lp2 * Math.exp(-tt / (0.25 * len)) * 40);
        g.t++;
        if (tt > 0.8) {   // done: close the gap, park the slot at the end
          for (let j = k; j < this.nBangs - 1; j++) bangs[j] = bangs[j + 1];
          bangs[--this.nBangs] = g; k--;
        }
      }
      // Flames: dark roaring noise swelling in about 25 ms and burning out
      // over a third of a second, a 55 Hz whoomp under it, and sparse crackles.
      for (let k = 0; k < this.nFlames; k++) {
        const f = flames[k];
        if (f.wait > 0) { f.wait--; continue; }
        const tt = f.t / sr;
        f.lp += 0.05 * ((rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1) - f.lp); f.lp2 += 0.08 * (f.lp - f.lp2);
        f.ph += 55 / sr;
        const env = Math.min(1, tt / 0.025) * Math.exp(-tt / 0.14);
        let c = 0;
        if (tt > 0.04 && tt < 0.35 && (rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 4294967296) < 0.004) c = (rs ^= rs << 13, rs ^= rs >>> 17, rs ^= rs << 5, (rs >>>= 0) / 2147483648 - 1) * 1.5;
        B += f.amp * (env * (f.lp2 * 26 + Math.sin(f.ph * TAU) * 0.9) + c * Math.exp(-tt / 0.2));
        f.t++;
        if (tt > 0.6) {
          for (let j = k; j < this.nFlames - 1; j++) flames[j] = flames[j + 1];
          flames[--this.nFlames] = f; k--;
        }
      }
      const top = T * s.gain + Math.tanh(B * 1.4) * 0.6 * mixP;
      if (this.toneOn) {
        L = Math.tanh(drive * L * 0.35) * satNorm * mixE; R = Math.tanh(drive * R * 0.35) * satNorm * mixE;
        const fl = this.toneL, fr = this.toneR;
        y = fl.b0 * L + fl.z1; fl.z1 = fl.b1 * L - fl.a1 * y + fl.z2; fl.z2 = fl.b2 * L - fl.a2 * y; outL[i] = y + top;
        y = fr.b0 * R + fr.z1; fr.z1 = fr.b1 * R - fr.a1 * y + fr.z2; fr.z2 = fr.b2 * R - fr.a2 * y; outR[i] = y + top;
      } else {
        outL[i] = Math.tanh(drive * L * 0.35) * satNorm * mixE + top;
        outR[i] = Math.tanh(drive * R * 0.35) * satNorm * mixE + top;
      }
    }
    rng.s = rs;
  }
}

// Warm-up: plays every sound path silently on a throwaway voice, so V8 has
// compiled all of it before anything is heard. Without this the first
// start-up, bang or blow-off runs in the slow interpreter while V8 compiles
// (and recompiles once a branch first runs), the audio thread misses its
// deadline and the sound cuts out; worst on a slow CPU like the Tesla's.
// cars: resolved cars (resolveCar). Returns how long it took (ms).
export function warmUp(sr, cars) {
  const t0 = Date.now();
  const pick = coverCars(cars);
  const L = new Float32Array(128), R = new Float32Array(128);
  const p = { rpm: 0, load: 0, overrun: 0, cut: 0, starter: 0, shaft: 0, horn: 0, pullAway: 0 };
  let v = null, k = 1;
  const play = (sec, set) => {
    Object.assign(p, set);
    v.setParams(p);
    for (let i = Math.ceil(k * sec * sr / 128); i > 0; i--) v.render(L, R, 128);
  };
  // A short drive through every state and event, so each branch runs. The
  // special vehicles' slow timers (rail joints, air pump, warp jump) are
  // wound forward so they go off too; this voice is thrown away.
  const tour = car => {
    v.configure(car);
    const e = car.engine, sp = v.special;
    play(0.03, { rpm: 200, load: 0.3, overrun: 0, cut: 0, starter: 1, shaft: 0, horn: 0, pullAway: 0 });
    play(0.03, { rpm: e.idle, load: 0.1, starter: 0 });
    play(0.02, { rpm: e.idle * 1.2, load: 0.35, pullAway: 0.7, shaft: 300 });
    v.boost = 1;   // skip the spool-up: full boost, ready to vent
    if (car.kind === 'steam') sp.railX = 17.99;
    play(sp ? 0.15 : 0.04, { rpm: e.redline, load: 1, pullAway: 0, shaft: 3000, horn: 1 });
    if (car.kind === 'warp') sp.jumpT = 1.15;
    play(0.06, { cut: 1 });
    v.event('up', 1); v.click(0.8);
    play(0.03, { cut: 0, load: 1 });
    v.event('lift', 1);
    play(0.06, { rpm: e.redline * 0.7, load: 0, overrun: 1, horn: 0 });
    v.event('down', 0.6); v.event('stop', 1); v.bang(1);
    if (car.kind === 'steam') { sp.rpm = 0; sp.pumpT = 1.09; }   // stopped: the air pump
    play(0.06, { rpm: e.idle * 1.6, shaft: 0 });
  };
  // 1. Every branch once, quickly, before V8 optimizes anything; then long
  //    enough for overlapping bangs and flames to finish and be cleared (V8
  //    optimizes along the way, with all of it seen). 2. The same again on a
  //    second voice, in optimized code: code that assumed only one voice, or
  //    anything that bailed out, is redone here rather than in the car.
  for (let round = 0; round < 2; round++) {
    v = new EngineVoice(sr, pick[0], 1 + round);
    v.setUser(0.75, 1);   // full crackle: afterfire pops are a dice roll per firing
    k = round === 0 ? 0.4 : 0.5;   // shortest that leaves nothing to recompile
    for (const car of pick) tour(car);
    if (round === 0) {
      // Overlapping bangs and flames, wound on to just before they end.
      v.configure(pick[0]);
      v.bang(1); v.bang(1); v.flame(1, 5); v.flame(1, 30);
      k = 1;
      play(0.05, { rpm: 3000, load: 0, overrun: 1 });
      // (Oldest furthest on, so they finish in turn and the gap-closing runs.)
      for (let i = 0; i < v.nBangs; i++) v.bangs[i].t = Math.round(0.78 * sr) + (v.nBangs - i) * 64;
      for (let i = 0; i < v.nFlames; i++) v.flames[i].t = Math.round(0.58 * sr) + (v.nFlames - i) * 64;
      play(0.06, {});
    }
  }
  return Date.now() - t0;
}

// A few cars that between them use every sound feature, plus one with the
// optional layers (cartoon, cam lope) switched on.
function coverCars(cars) {
  const feats = car => {
    const s = car.sound, f = ['kind:' + (car.kind || 'piston')];
    if (car.kind && car.kind !== 'piston') return f;
    if (s.turbo.amt > 0) f.push('turbo:' + s.turbo.bov);
    if (s.antilag > 0) f.push('antilag');
    if (s.gunshot && s.gunshot.amt > 0) f.push('gunshot');
    if (s.gunshot && s.gunshot.flame > 0) f.push('flame');
    if (s.jake && s.jake.amt > 0) f.push('jake');
    if (s.airBrake > 0) f.push('air');
    if (s.blower.amt > 0) f.push('blower');
    if (s.gearWhine.amt > 0) f.push('gear');
    if ((car.cycle || 720) !== 720) f.push('2stroke');
    if (s.header) f.push('header');
    if (s.cross > 0) f.push('cross');
    if (s.overrunRough > 0) f.push('ovrough');
    if (s.crackle > 0) f.push('crackle');
    if (s.toneHz > 0) f.push('tone');
    f.push('banks' + Math.max(...car.firing.map(x => x[1] + 1)), 'pipes' + s.pipes.length);
    return f;
  };
  const need = new Set(), pick = [];
  for (const c of cars) for (const f of feats(c)) need.add(f);
  while (need.size) {
    let best = null, gain = 0;
    for (const c of cars) {
      const g = feats(c).filter(f => need.has(f)).length;
      if (g > gain) { best = c; gain = g; }
    }
    if (!best) break;
    pick.push(best);
    for (const f of feats(best)) need.delete(f);
  }
  const base = pick.find(c => !c.kind || c.kind === 'piston');
  if (base) pick.push({ ...base, sound: { ...base.sound, toon: 1, lope: 0.1 } });
  return pick;
}
