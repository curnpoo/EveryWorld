// drivetrain.js — what a combustion car would be doing at a given road speed.
//
// Input: road speed and acceleration (from GPS, or from the test-drive
// physics below), plus the Rev button. Output for the synthesizer: rpm, load,
// overrun, ignition cut, starter and gearbox output speed, and for the
// dashboard: gear and throttle.
//
// In GPS mode the throttle isn't known, so it's estimated from how hard the
// real car (HOST, the Tesla) accelerates compared with what it could do at
// that speed: flooring the Tesla is full throttle, cruising a light load.
//
// Sport mode holds the lowest gear that fits: late upshifts, early
// downshifts, revs kept high. Out of Sport it drives like Eco: always the
// tallest gear that keeps the engine above its lugging floor, skipping gears
// on the way up, and a kickdown (several gears at once) on a deep pedal.

import { GEARBOX, HOST } from './cars.js';

const G = 9.81;
const AIR = 1.2;

function clamp(x, lo, hi) { return x < lo ? lo : x > hi ? hi : x; }
function lerp(a, b, t) { return a + (b - a) * t; }
function smoothstep(x) { x = clamp(x, 0, 1); return x * x * (3 - 2 * x); }
function follow(dt, tau) { return 1 - Math.exp(-dt / Math.max(tau, 1e-4)); }

// Start-up (per car, engine.start): the starter cranks, the first cylinders
// catch unevenly, the engine flares (and may bark), then settles to idle.
const START = { crank: 0.75, crankRpm: 190, flare: 1.9, bark: 0, settle: 1.6 };
const FLARE_S = 0.4;
// How long a lift-off (or getting back on it) must last before the sound
// follows: just enough to ignore GPS noise.
const COAST_HOLD_S = 0.1, ON_HOLD_S = 0.1;
// Gentle slowing turns into engine braking only after this long; and the
// least throttle while cruising (sounds like light throttle, not coasting).
const GENTLE_COAST_S = 3.5, CRUISE_MIN = 0.04;
// With a phone's motion sensor feeding the Tesla (direct): its reading is
// clean and immediate, so a lift is engine braking as soon as the car is
// clearly slowing, with no hold; and the revs flex with the pedal like a
// real drivetrain (converter or clutch slip, wind-up): a few percent with
// throttle, plus a kick on a fresh push that settles.
const DIRECT_OFF = 0.5, DIRECT_ON = 0.5;   // m/s² slowing: engine braking from here; back on at this share of it
const FLEX = { auto: 0.06, cvt: 0.06, dct: 0.04, seq: 0.035, manual: 0.035 };
const FLEX_KICK = 0.1, FLEX_KICK_S = 0.5;

export class Drivetrain {
  constructor(engine) {
    this.intensity = 0.5;
    this.sport = false;
    this.direct = false;   // a phone's motion sensor is feeding the Tesla (app.js)
    this.topMph = 145;     // null: real ratios and real top speed
    this.setEngine(engine);
    this.reset();
  }

  setEngine(engine) {
    this.e = engine;
    this.box = GEARBOX[engine.gearbox] || GEARBOX.dct;
    const n = engine.ratios.length;
    if (this.gear > n) this.gear = n;
    this._fitGears();
  }

  // Top speed: null runs the real ratios up to the car's real top speed.
  // A number (mph) shortens every ratio by the same factor so top gear
  // reaches the redline at that speed, which becomes the top speed.
  // Never taller than the real car.
  setTopSpeed(mph) { this.topMph = mph || null; this._fitGears(); }
  _fitGears() {
    const e = this.e, n = e.ratios.length;
    const kAt = mph => Math.max(e.fitTaller ? 0.05 : 1, e.redline / ((mph / 2.23694) / e.tire * 60 * e.ratios[n - 1] * e.final));
    this.k = this.topMph ? kAt(this.topMph) : 1;
    const vLimit = e.limiter / 60 / (e.ratios[n - 1] * e.final * this.k) * e.tire;
    const want = this.topMph || e.topMph;
    this.vMax = want ? Math.min(vLimit, want / 2.23694) : Math.min(vLimit, this._dragTop());
  }

  _dragTop() {
    const e = this.e, P = e.peakKw * 1000 * 0.88;
    let v = 50;
    for (let i = 0; i < 40; i++) v = Math.cbrt(Math.max(1, (P - e.crr * e.mass * G * v)) / (0.5 * AIR * e.cdA));
    return v;
  }

  reset() {
    this.t = 0;
    this.running = false;
    this.startT = 99;
    this.rpm = 0;
    this.gear = 1;
    this.shift = null;
    this.lastShift = -9;
    this.kicking = false;
    this.throttle = 0;
    this.thSlow = 0;       // throttle, slow (the flex kick on a fresh push)
    this.coastLatch = false;
    this.gentleT = 0;
    this.load = 0;
    this.overrun = 0;
    this.pullAway = 0;
    this.cut = 0;
    this.starter = 0;
    this.limiterT = 0;
    this.revHeld = 0;
    this.clicks = [];
    this.events = [];      // 'up' shifts and lift-offs, for the backfire
    this.speed = 0;
    this.accel = 0;
    this.horn = 0;
    this.moving = false;
  }

  start() {
    this.reset();
    this.running = true;
    this.startT = 0;
    this.barked = false;
  }

  stop() { this.running = false; this.rpm = 0; }

  wheelRpm(v) { return v / this.e.tire * 60; }
  gearRpm(g, v) { return this.wheelRpm(v) * this.e.ratios[g - 1] * this.e.final * this.k; }

  resist(v) { return (0.5 * AIR * this.e.cdA * v * v) / this.e.mass + this.e.crr * G; }
  availAccel(v) {
    const e = this.e;
    return Math.min(e.maxAccel, e.peakKw * 1000 * 0.88 / (e.mass * Math.max(v, 3)));
  }

  // Shift points for a throttle: gentle driving changes up early, full
  // throttle near the redline. Intensity moves both later.
  upRpm(th) {
    const [cruise, full] = this.e.shift;
    if (this.sport) return lerp(full * 0.86, full, smoothstep(th * 1.5));
    // Eco: a relaxed automatic. Light throttle changes up early (about 30%
    // of the redline), and the shift point climbs with every bit of pedal.
    const light = Math.max(Math.min(cruise, this.e.redline * 0.32), this.ecoFloor() * 1.4);
    return lerp(light, full, Math.pow(clamp(th / 0.9, 0, 1), 1.6));
  }
  // Eco: the lowest revs it will pull from before changing down.
  ecoFloor() {
    const e = this.e;
    return Math.max(e.downRpm || 0, e.idle * 1.6, e.redline * 0.18);
  }
  // Change down below this. Braking harder changes down earlier, the way a
  // DCT or a driver does; Sport keeps the engine in its power band.
  downRpm(th, accel) {
    const e = this.e, red = e.redline;
    const base = Math.max(e.downRpm || 0, e.idle * 1.6, red * 0.22);
    if (this.sport) return Math.max(base, red * 0.52);
    const floor = this.ecoFloor();
    const braking = clamp(-(accel || 0) / 3.5, 0, 1);
    return Math.max(lerp(floor, red * 0.5, th * th * th), lerp(floor, red * 0.3, braking));
  }

  // Cruising holds a light throttle through small speed wobbles; it only
  // goes off-throttle (overrun) on a real slow-down, and comes back on
  // before speed has to climb again (hysteresis, so it cannot flutter).
  estimateThrottle(v, a, dt) {
    if (v < 0.4 && a < 0.2) return 0;
    const h = HOST;
    const resist = (0.5 * AIR * h.cdA * v * v) / h.mass + h.crr * G;
    const avail = Math.min(h.maxAccel, h.peakKw * 1000 * 0.9 / (h.mass * Math.max(v, 3)));
    const gain = lerp(0.8, 1.8, this.intensity);
    if (this.direct) {
      const offAt = -Math.max(DIRECT_OFF, resist + 0.25);
      this.coastLatch = this.coastLatch ? a < offAt * DIRECT_ON : a < offAt;
      this.gentleT = 0; this.coastPend = 0;
      if (this.coastLatch) return 0;
    }
    // Two kinds of slowing down. Gentle (a partial lift): keep a light,
    // steady throttle; only after GENTLE_COAST_S of it does the engine go
    // to engine braking. Firm (a real lift-off with regen, or braking):
    // engine braking straight away.
    const gentleAt = -Math.max(0.6, resist + 0.25), firmAt = -Math.max(1.0, resist + 0.6);
    const onAt = gentleAt * 0.4;
    // (Counts while slowing gently, pauses while hovering around the line,
    // starts over only once back on the throttle.)
    if (a < gentleAt) this.gentleT = (this.gentleT || 0) + (dt || 0);
    else if (a > onAt) this.gentleT = 0;
    // A change of state must hold briefly first, so GPS noise right at a
    // threshold can't flick between the two.
    const want = this.direct ? false : this.coastLatch ? a < onAt : a < firmAt || this.gentleT > GENTLE_COAST_S;
    this.coastPend = want !== this.coastLatch ? (this.coastPend || 0) + (dt || 0) : 0;
    if (!this.direct && this.coastPend > (want ? COAST_HOLD_S : ON_HOLD_S)) { this.coastLatch = want; this.coastPend = 0; }
    if (this.coastLatch) return 0;
    let th = (a + resist) / avail * gain;
    // Above walking pace, short of engine braking: keep at least half the
    // throttle that holds a steady speed, and never so little that it
    // sounds like coasting.
    if (v > 4) th = Math.max(th, 0.5 * resist / avail * gain, CRUISE_MIN);
    // Pulling away, the Tesla's instant torque gives a lot of acceleration
    // for a little pedal, so the same acceleration means less throttle near
    // a standstill; by about 15 mph it reads as before. Flooring it is still
    // full throttle.
    const launch = 1 - smoothstep(v / 7);
    return Math.pow(clamp(th, 0, 1), 0.8 + 0.5 * launch);
  }

  // Test drive: advance the car under pedal input; returns { v, a }.
  simulate(dt, gas, brake) {
    const v = this.speed;
    let a = gas * this.availAccel(v) * (this.shift && this.shift.kind === 'up' ? (this.box.clutch ? 0 : 0.35) : 1);
    a -= this.resist(v);
    if (gas < 0.05 && this.gear && v > 1) a -= 0.6 * clamp(this.rpm / this.e.redline, 0.2, 1);
    a -= brake * 9;
    if (v >= this.vMax && a > 0) a = 0;
    if (!this.shift && this.rpm >= this.e.limiter - 30 && a > 0) a *= 0.15;
    let nv = v + a * dt;
    if (nv < 0) { nv = 0; a = Math.max(a, -v / Math.max(dt, 1e-3)); }
    return { v: nv, a: clamp(a, -12, 15) };
  }

  // input: { speed (m/s), accel (m/s²), throttle (0..1, test drive only), rev (bool) }
  update(dt, input) {
    const e = this.e;
    this.t += dt;
    this.speed = input.speed;
    this.accel = input.accel || 0;
    if (!this.running) {
      this.rpm += (0 - this.rpm) * follow(dt, 0.3);
      return this.out(0);
    }
    const v = input.speed;
    const stopped = v < 0.3;
    const rev = input.rev && stopped;

    // Throttle: test drive knows it; GPS estimates it from acceleration.
    let th = input.throttle != null ? input.throttle : this.estimateThrottle(v, input.accel, dt);
    const k = th > this.throttle ? follow(dt, 0.05) : follow(dt, 0.06);
    this.throttle += (th - this.throttle) * k;
    th = this.throttle;

    this.revHeld += ((rev ? 1 : 0) - this.revHeld) * follow(dt, rev ? 0.05 : 0.04);

    // Start-up sequence.
    this.startT += dt;
    const st = this.e.start || START;
    if (this.startT < st.crank + FLARE_S + st.settle) return this._starting(dt, st);
    this.starter = 0;

    let target, tau, load;
    let cut = 0;

    // Coming to a stop (air brakes).
    if (v > 3) this.moving = true;
    else if (stopped && this.moving) { this.moving = false; this.events.push({ type: 'stop', strength: 1 }); }
    // Steam: Rev blows the whistle instead of revving.
    this.horn = e.mode === 'direct' ? this.revHeld : 0;

    if (stopped && e.mode === 'direct') {
      this.shift = null;
      this.gear = 1;
      target = 0; tau = 0.2; load = 0;
    } else if (stopped) {
      // Standstill: idling, or free-revving on the Rev button.
      this.shift = null;
      this.gear = 1;
      const want = this.revHeld > 0.02;
      target = want ? lerp(e.idle, e.limiter + 200, this.revHeld) : e.idle;
      tau = target > this.rpm ? e.revUp : e.revDown;
      load = want ? lerp(0.35, 1, this.revHeld) : 0.1;
      if (!want && this.rpm > e.idle * 1.3) load = 0;
    } else if (e.mode) {
      ({ target, tau, load } = this._noGearbox(v, th));
      this.gear = 1;
    } else if (this.box.cvt) {
      // CVT: the throttle sets the revs; the belt ratio slides between its
      // ends (low for pulling away, high for top speed).
      const lowR = this.gearRpm(1, v), highR = this.gearRpm(2, v);
      const want = th < 0.03 ? e.idle : lerp(e.shift[0], e.shift[1], smoothstep(th * 1.3));
      const slip = e.idle + (e.launch - e.idle) * smoothstep(th * 1.2) * (1 - smoothstep(v / 6));
      target = Math.max(clamp(want, highR, lowR), slip, e.idle);
      tau = target > this.rpm ? 0.12 : 0.2;
      load = th < 0.03 ? 0 : 0.12 + 0.88 * th;
      this.gear = 1;
    } else {
      this._chooseGear(dt, v, th);
      const locked = this.gearRpm(this.gear, v);
      // Clutch slip / torque converter: pulling away holds the revs up.
      const slip = e.idle + (e.launch - e.idle) * smoothstep(th * 1.2) * (1 - smoothstep(v / 8));
      // Direct: the drivetrain flexes with the pedal (see FLEX).
      this.thSlow += (th - this.thSlow) * follow(dt, FLEX_KICK_S);
      const flex = this.direct && !this.shift ? (FLEX[e.gearbox] ?? 0.03) * th + FLEX_KICK * Math.max(0, th - this.thSlow) : 0;
      target = Math.max(locked * (1 + flex), slip, e.idle);
      tau = 0.04;
      load = th < 0.03 ? 0 : 0.12 + 0.88 * th;
      if (this.shift) {
        const s = this.shift, box = this.box;
        s.t += dt;
        const x = clamp(s.t / s.dur, 0, 1);
        const to = Math.max(this.gearRpm(s.to, v), e.idle);
        if (s.kind === 'up') {
          if (box.clutch) {
            // Clutch in: the revs fall freely, then the clutch bites.
            target = x < 0.7 ? Math.max(to, e.idle) : to;
            tau = x < 0.7 ? e.revDown * 0.9 : 0.06;
            load = x < 0.75 ? 0.02 : load;
          } else {
            target = lerp(s.from, to, smoothstep(x));
            tau = 0.03;
            if (box.cut && s.th > 0.45 && s.t < box.cut) cut = 1;
            if (!box.cut) load *= 0.55;
          }
        } else {
          // Downshift: blip the throttle to match the revs.
          target = lerp(s.from, to * 1.03, smoothstep(x * 1.4));
          tau = box.clutch ? 0.08 : 0.04;
          if (x < 0.7) load = Math.max(load, box.blip * (0.5 + 0.5 * this.intensity));
        }
        if (s.t >= s.dur) {
          this.gear = s.to;
          if (box.clutch) this.clicks.push(0.5 + 0.3 * s.th);
          this.shift = null;
          this.lastShift = this.t;
        }
      }
    }

    // Idle: a slow, gentle wander, ±5 rpm at most (no cam lope).
    if (e.idle > 0 && !e.mode && target < e.idle * 1.15) target += 4 * Math.sin(this.t * 0.7) + Math.sin(this.t * 1.9);

    this.rpm += (target - this.rpm) * follow(dt, tau);

    // Rev limiter: cut the spark, let the revs drop, fire again.
    if (this.limiterT > 0) {
      this.limiterT -= dt;
      cut = 1;
      this.rpm = Math.min(this.rpm, e.limiter) - dt * 4000;
    } else if (this.rpm >= e.limiter) {
      this.limiterT = 0.055;
      this.rpm = e.limiter;
      cut = 1;
    }
    if (this.rpm < 0) this.rpm = 0;

    const coasting = load < 0.02 && this.rpm > e.idle * 1.25;
    // Snapping off the throttle from high revs: one bang.
    if (coasting && this.overrun < 0.05 && this.rpm > e.redline * 0.35 && this.peakTh > 0.45) {
      this.events.push({ type: 'lift', strength: Math.min(1, 0.4 + 0.6 * this.peakTh) });
    }
    this.peakTh = Math.max(th, (this.peakTh || 0) - dt * 1.5);
    this.overrun += ((coasting ? 1 : 0) - this.overrun) * follow(dt, 0.12);
    this.load += (load - this.load) * follow(dt, load > this.load ? 0.04 : 0.08);
    // Pulling away gently from a stop: 1 at walking pace on a light pedal,
    // gone by about 10 mph or once the pedal is past about half. The synth
    // eases the low boom with it, so a chill start isn't louder than cruising
    // (the revs sit right on the boom's resonance there); getting on it is
    // untouched.
    const pull = !e.mode && !rev && (v > 0.3 || th > 0.03)
      ? (1 - smoothstep(v / 4.5)) * (1 - smoothstep((th - 0.35) / 0.3)) : 0;
    this.pullAway += (pull - this.pullAway) * follow(dt, 0.15);
    this.cut = cut;
    return this.out(stopped ? 0 : this.wheelRpm(v) * e.final);
  }

  // Vehicles without a gearbox. Turbines: the spool follows the throttle and
  // climbs with road speed. Direct (steam): the wheels drive the cylinders.
  _noGearbox(v, th) {
    const e = this.e;
    if (e.mode === 'direct') {
      return { target: this.gearRpm(1, v), tau: 0.03, load: th < 0.03 ? 0.05 : 0.15 + 0.85 * th };
    }
    const f = clamp(0.62 * th + 0.45 * clamp(v / this.vMax, 0, 1), 0, 1);
    const target = lerp(e.idle, e.redline, f);
    return { target, tau: target > this.rpm ? e.revUp : e.revDown, load: th < 0.03 ? 0 : 0.1 + 0.9 * th };
  }

  _chooseGear(dt, v, th) {
    const e = this.e, n = e.ratios.length;
    if (this.shift) return;
    // A real jump in speed (first GPS fix while already moving): pick a
    // sensible gear at once. Normal acceleration always shifts through.
    const jumped = Math.abs(v - (this.lastV ?? v)) > 6;
    this.lastV = v;
    if (jumped && this.gearRpm(this.gear, v) > e.limiter) {
      let g = this.gear;
      while (g < n && this.gearRpm(g, v) > this.upRpm(th)) g++;
      this.gear = g;
      return;
    }
    const since = this.t - this.lastShift;
    const r = this.gearRpm(this.gear, v);
    if (th < 0.45) this.kicking = false;
    const down = this.downRpm(th, this.accel);
    // Each gear is held long enough to hear it.
    const dwell = e.minGearS || 0.6;
    // On the limiter the wait is cut short: bouncing there sounds like air.
    const ready = since > dwell || (since > 0.35 && r >= e.redline);
    if (ready && this.gear < n && r > this.upRpm(th) && this.gearRpm(this.gear + 1, v) > down * 1.05) {
      // Eco on a light pedal: gearboxes that can (skipShift) go straight to
      // the tallest gear that pulls; the rest go up one at a time.
      let to = this.gear + 1;
      if (e.skipShift && !this.sport && th < 0.3) while (to < n && this.gearRpm(to + 1, v) > down * 1.05) to++;
      this._beginShift('up', to, th);
    } else if (!this.sport && since > 0.3 && th > 0.55 && this.gear > 1) {
      // Eco kickdown: deep pedal drops to the lowest gear that keeps the
      // revs under the full-throttle shift point; in one shift with
      // skipShift, otherwise a gear at a time until it gets there.
      let to = this.gear;
      while (to > 1 && this.gearRpm(to - 1, v) < Math.min(this.upRpm(th) * 0.9, e.limiter * 0.9)) to--;
      if (!e.skipShift && to < this.gear) to = this.gear - 1;
      if (to < this.gear && (this.kicking || this.gearRpm(this.gear, v) < this.upRpm(th) * 0.6)) {
        this.kicking = true;
        this._beginShift('down', to, th);
      } else if (r < down && this.gearRpm(this.gear - 1, v) < e.limiter * 0.95) this._beginShift('down', this.gear - 1, th);
    } else if (since > 0.3 && this.gear > 1) {
      const lower = this.gearRpm(this.gear - 1, v);
      const kick = th > 0.8 && lower < e.shift[1] * 0.9;
      if ((r < down || kick) && lower < this.upRpm(th) * 0.97 && lower < e.limiter * 0.95) this._beginShift('down', this.gear - 1, th);
    }
  }

  _beginShift(kind, to, th) {
    const box = this.box;
    this.shift = { kind, to, from: this.rpm, t: 0, th, dur: kind === 'up' ? box.up * lerp(1.15, 0.85, th) : box.down };
    if (kind === 'up' && th > 0.4) this.events.push({ type: 'up', strength: th });
    if (kind === 'down' && this.sport) this.events.push({ type: 'down', strength: 0.5 });
  }

  _starting(dt, st) {
    const e = this.e, t = this.startT;
    let target, load, cut = 0, tau = 0.12;
    this.overrun = 0;
    if (t < st.crank) {
      // Cranking: the crank slows on every compression stroke.
      this.starter = 1;
      const strokes = st.crankRpm / 60 * (e.cyl || 8) / 2;
      target = st.crankRpm * (1 + 0.28 * Math.sin(t * Math.PI * 2 * strokes));
      tau = 0.015;
      load = 0;
      cut = 1;
      // Near the end the first cylinders catch, unevenly.
      if (t > st.crank * 0.7 && Math.random() < 0.35) { cut = 0; load = 0.6; }
    } else if (t < st.crank + FLARE_S) {
      // Caught: it flares up, the starter lets go.
      this.starter = 0;
      target = e.idle * st.flare;
      tau = e.revUp * 0.8;
      load = 0.9;
      if (!this.barked && st.bark > 0 && t > st.crank + FLARE_S * 0.6) {
        this.barked = true;
        this.events.push({ type: 'start', strength: st.bark });
      }
    } else {
      // Settle back to idle; cars with a bark crackle on the way down.
      this.starter = 0;
      const x = (t - st.crank - FLARE_S) / st.settle;
      target = lerp(e.idle * Math.min(st.flare, 1.35), e.idle, smoothstep(x));
      tau = e.revDown;
      load = x < 0.15 ? 0 : 0.15;
      if (st.bark > 0.3 && x < 0.25) this.overrun = 1;
    }
    this.rpm += (target - this.rpm) * follow(dt, tau);
    this.load += (load - this.load) * follow(dt, 0.03);
    this.cut = cut;
    this.gear = 1;
    return this.out(0);
  }

  out(shaft) {
    return {
      rpm: this.rpm, load: this.load, overrun: this.overrun, cut: this.cut,
      starter: this.starter, shaft, gear: this.gear, throttle: this.throttle, horn: this.horn,
      pullAway: this.pullAway, shifting: !!this.shift
    };
  }
}
