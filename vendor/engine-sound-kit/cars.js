// cars.js — car profiles.
//
// engine:  drivetrain and physics (drivetrain.js). Power, torque, ratios and
//          tyre sizes are the real car's; tire is the rolling circumference
//          in metres.
// firing:  [crank angle °, exhaust bank] in firing order, over one 720° cycle.
// sound:   synthesizer settings (synth.js), merged over DEFAULT_SOUND. These
//          are starting points to be tuned by ear (the Tune panel).
//
// engine.start: crank (s), crankRpm, flare (× idle on catching), bark (0..1,
// a backfire as it catches), settle (s back down to idle).
//
// gearbox: 'dct' dual clutch, 'seq' single-clutch automated manual (slow,
// violent upshifts), 'auto' torque-converter automatic, 'manual' (clutch in
// on every shift, rev-matched downshifts), 'seamless' (race).
// engine.skipShift: the gearbox can jump several gears in one shift (Eco
// upshifts and kickdown); without it every shift is one gear.

// The car this actually runs in. The throttle is judged against what it can
// do, and road speed tops out at its top speed. 2020 Model 3 Performance.
export const HOST = { name: 'Tesla Model 3 Performance', mass: 1850, peakKw: 340, maxAccel: 9.5, cdA: 0.58, crr: 0.01, topMph: 145 };

export const DEFAULT_SOUND = {
  pipes: [{ bank: 0, ms: 5, pan: -0.5 }, { bank: 1, ms: 5.4, pan: 0.5 }],
  reflect: -0.25,
  damp: 0.5,
  cross: 0,                       // X/H-pipe: 0 separate banks, 1 fully merged
  header: null,                   // extra delay per firing (ms): unequal-length headers
  pulse: { idleMs: 3.5, topMs: 0.8 },
  noise: 0.5,
  rough: 0.04,
  trim: 0.05,
  lope: 0,
  idleRoll: 0.15,                  // idle unevenness and slow sub beats: 1 full, 0 glassy smooth
  muffler: { closedHz: 900, openHz: 6000 },
  body: [
    { hz: 90, db: 4, q: 1 },
    { hz: 400, db: 2, q: 1.5 },
    { hz: 1200, db: 0, q: 2 },
    { hz: 3000, db: 0, q: 2 }
  ],
  intake: { amt: 0.1, hz: 800, q: 1.2 },
  bass: { hz: 110, db: 5 },        // low shelf below about this
  thump: { amt: 1, hz: 48, q: 1.4, sub: 1 }, // low boom rung by every firing, plus sub (10-60 Hz)
  overrunRough: 0,                 // unevenness when coasting (0 = steady drone)
  overrunDark: 0.7,                // how much lower / darker the tone gets when coasting
  starter: { amt: 1, pitch: 1 },   // starter motor level and pitch
  mech: 0.03,
  turbo: { amt: 0, spoolRpm: 2500, spoolS: 0.6, damp: 0.5, whistle: 0.02, whistleHz: [2500, 7000], hiss: 0.05, bov: 'none', bovAmt: 0.3 }, // psi: full boost on the dash gauge (default 20)
  blower: { amt: 0, ratio: 20 },
  gearWhine: { amt: 0, ratio: 30 },
  crackle: 0.5,
  popAmt: 0.5,
  bang: 0.3,                      // backfire on hard upshifts / sharp lift-off
  bangBurst: [1, 2],               // bangs per backfire (min, max)
  bangGapMs: 40,                   // time between bangs in a burst
  bangCrack: 0.6,                  // how sharp the crack on top of each boom is
  jake: { amt: 0 },                // engine (jake) brake: cylinders bark on overrun
  airBrake: 0,                     // air-brake hiss when coming to a stop
  gunshot: { amt: 0, flame: 0 },   // lift-off gunshot bang, optionally with flames
  antilag: 0,                      // anti-lag: turbo stays spooled off throttle, constant bangs
  toon: 0,                         // cartoon layers: kazoo buzz, springy revs, honk instead of bangs
  drive: 2.5,
  gain: 0.5
};

// Shift times are kept short for every gearbox: the Tesla's speed keeps
// climbing smoothly through a shift, so a long, realistic one (a slushbox's
// 0.4 s, a manual's 0.6 s) feels like lag. Each type keeps its character
// (cut, blip, clutch click), just quickly.
export const GEARBOX = {
  dct:      { up: 0.12, down: 0.22, cut: 0.07, blip: 0.6 },
  seq:      { up: 0.12, down: 0.2, cut: 0.08, blip: 0.7 },
  auto:     { up: 0.15, down: 0.2, cut: 0, blip: 0.35 },
  manual:   { up: 0.18, down: 0.22, cut: 0, blip: 0.6, clutch: true },
  seamless: { up: 0.04, down: 0.08, cut: 0.03, blip: 0.6 },
  // Belt-drive CVT (scooters): revs set by the throttle, ratio slides.
  cvt:      { up: 0.1, down: 0.1, cut: 0, blip: 0, cvt: true },
  // No gearbox (turbines, steam): the drivetrain handles these by engine.mode.
  direct:   { up: 0.1, down: 0.1, cut: 0, blip: 0 }
};

// Even firing with banks alternating, e.g. flat-plane V8, V10, V12, flat-six.
function alternate(n) {
  const out = [];
  for (let i = 0; i < n; i++) out.push([i * 720 / n, i % 2]);
  return out;
}
// Even firing into one collector (inline engines, boxer with a Y-pipe).
function single(n) {
  const out = [];
  for (let i = 0; i < n; i++) out.push([i * 720 / n, 0]);
  return out;
}
// Cross-plane V8: every firing order gives the same left/right pattern
// L-R-L-R-R-L-R-L (rotated), so each bank fires unevenly: the burble.
function crossplane(banks) {
  return banks.map((b, i) => [i * 90, b]);
}
const CROSSPLANE = [0, 1, 0, 1, 1, 0, 1, 0];

export const CARS = [
  // ── Naturally aspirated ───────────────────────────────────────────────
  {
    id: 'gt3', group: 'na',
    name: 'Porsche 911 GT3', tagline: '992 · 4.0 L flat-six · 9,000 rpm · 7-speed PDK',
    engine: { topMph: 198,
      idle: 850, redline: 9000, limiter: 9000, launch: 4000, revUp: 0.12, revDown: 0.26,
      shift: [3800, 8900], start: { crank: 0.6, crankRpm: 200, flare: 2.3, bark: 0.15, settle: 1.6 },
      gearbox: 'dct',
      peakKw: 375, mass: 1435, cdA: 0.68, crr: 0.011, maxAccel: 9.5,
      disp: 4.0,
      ratios: [3.75, 2.38, 1.72, 1.34, 1.11, 0.96, 0.84], final: 3.97, tire: 2.25
    },
    // 1-6-2-4-3-5; cylinders 1-3 on one bank, 4-6 on the other.
    firing: alternate(6),
    sound: {
      pipes: [{ bank: 0, ms: 3.2, pan: -0.4 }, { bank: 1, ms: 3.4, pan: 0.4 }],
      reflect: -0.2, damp: 0.53,
      pulse: { idleMs: 2.8, topMs: 0.6 },
      noise: 0.71, rough: 0.025, trim: 0.03, lope: 0,
      muffler: { closedHz: 1200, openHz: 9000 },
      body: [{ hz: 140, db: 3, q: 1.0 }, { hz: 650, db: 4, q: 1.3 }, { hz: 1800, db: 5, q: 1.3 }, { hz: 3500, db: 2, q: 1.3 }],
      intake: { amt: 0.16, hz: 1400, q: 1.2 },
      bass: { hz: 120, db: 4 }, thump: { amt: 1, hz: 52, q: 1.4, sub: 1.6 }, overrunRough: 0, starter: { amt: 0.9, pitch: 1.1 },
      mech: 0.035, crackle: 0, popAmt: 0.45, bang: 0.2, drive: 3.1, gain: 0.34
    }
  },
  {
    id: 'f458', group: 'na',
    name: 'Ferrari 458 Italia', tagline: '4.5 L flat-plane V8 · 9,000 rpm · 7-speed DCT',
    engine: { topMph: 202,
      idle: 1000, redline: 9000, limiter: 9000, launch: 4000, revUp: 0.1, revDown: 0.22,
      shift: [3500, 8900], start: { crank: 0.55, crankRpm: 210, flare: 3.0, bark: 0.3, settle: 1.8 },
      gearbox: 'dct',
      peakKw: 419, mass: 1485, cdA: 0.7, crr: 0.011, maxAccel: 9.5,
      disp: 4.5,
      ratios: [3.08, 2.19, 1.63, 1.29, 1.03, 0.84, 0.69], final: 4.44, tire: 2.245
    },
    // 1-5-3-7-4-8-2-6, flat crank: banks strictly alternate, each fires every
    // 180° like an inline-four: the shriek instead of a burble.
    firing: alternate(8),
    sound: {
      pipes: [{ bank: 0, ms: 3.0, pan: -0.4 }, { bank: 1, ms: 3.1, pan: 0.4 }],
      reflect: -0.2, damp: 0.5,
      pulse: { idleMs: 2.6, topMs: 0.5 },
      noise: 0.73, rough: 0.025, trim: 0.03, lope: 0,
      muffler: { closedHz: 1400, openHz: 10000 },
      body: [{ hz: 120, db: 3, q: 1.0 }, { hz: 800, db: 4, q: 1.3 }, { hz: 2400, db: 6, q: 1.3 }, { hz: 4200, db: 3, q: 1.3 }],
      intake: { amt: 0.16, hz: 1600, q: 1.2 },
      bass: { hz: 110, db: 4 }, thump: { amt: 1, hz: 55, q: 1.4, sub: 1.5 }, overrunRough: 0, starter: { amt: 0.9, pitch: 1.15 },
      mech: 0.035, crackle: 0, popAmt: 0.5, bang: 0.3, drive: 3.3, gain: 0.34
    }
  },
  {
    id: 'lfa', group: 'na',
    name: 'Lexus LFA', tagline: '4.8 L V10 · 9,000 rpm · 6-speed ASG',
    engine: { topMph: 202,
      idle: 1050, redline: 9000, limiter: 9300, launch: 4200, revUp: 0.06, revDown: 0.14,
      shift: [3500, 8900], start: { crank: 0.5, crankRpm: 220, flare: 2.7, bark: 0.2, settle: 1.2 },
      gearbox: 'seq',
      peakKw: 412, mass: 1480, cdA: 0.66, crr: 0.011, maxAccel: 9.5,
      disp: 4.8,
      ratios: [3.231, 2.188, 1.609, 1.233, 1.0, 0.806], final: 4.3, tire: 2.17
    },
    // 1-6-5-10-2-7-3-8-4-9, even every 72°, banks alternate.
    firing: alternate(10),
    sound: {
      pipes: [{ bank: 0, ms: 2.6, pan: -0.35 }, { bank: 1, ms: 2.7, pan: 0.35 }],
      reflect: -0.19, damp: 0.49,
      pulse: { idleMs: 2.4, topMs: 0.45 },
      noise: 0.69, rough: 0.02, trim: 0.025, lope: 0,
      muffler: { closedHz: 1500, openHz: 11000 },
      body: [{ hz: 150, db: 2, q: 1.0 }, { hz: 900, db: 4, q: 1.3 }, { hz: 2000, db: 6, q: 1.3 }, { hz: 3600, db: 5, q: 1.3 }],
      intake: { amt: 0.16, hz: 1800, q: 1.3 },
      bass: { hz: 120, db: 3.5 }, thump: { amt: 1, hz: 55, q: 1.4, sub: 1.5 }, overrunRough: 0, starter: { amt: 0.8, pitch: 1.2 },
      mech: 0.03, crackle: 0, popAmt: 0.4, bang: 0.2, drive: 3.1, gain: 0.36
    }
  },
  {
    id: 'm3e90', group: 'na',
    name: 'BMW M3 (E90)', tagline: 'S65 4.0 L V8 · 8,400 rpm · individual throttles · 7-speed DCT',
    engine: { topMph: 180,
      idle: 800, redline: 8300, limiter: 8400, launch: 3500, revUp: 0.12, revDown: 0.25,
      shift: [3200, 8200], start: { crank: 0.7, crankRpm: 190, flare: 2.1, bark: 0.15, settle: 1.6 },
      gearbox: 'dct',
      peakKw: 309, mass: 1655, cdA: 0.68, crr: 0.011, maxAccel: 7.5,
      disp: 4.0,
      ratios: [4.055, 2.396, 1.582, 1.192, 0.975, 0.813, 0.672], final: 3.15, tire: 2.10
    },
    // 1-5-4-8-6-3-7-2, cylinders 1-4 / 5-8: cross-plane.
    firing: crossplane(CROSSPLANE),
    sound: {
      pipes: [{ bank: 0, ms: 4.0, pan: -0.5 }, { bank: 1, ms: 4.3, pan: 0.5 }],
      reflect: -0.15, damp: 0.55,
      pulse: { idleMs: 3.0, topMs: 0.6 },
      noise: 0.73, rough: 0.035, trim: 0.05, lope: 0,
      muffler: { closedHz: 1000, openHz: 8000 },
      body: [{ hz: 100, db: 4, q: 1.0 }, { hz: 500, db: 3, q: 1.3 }, { hz: 1500, db: 5, q: 1.3 }, { hz: 3000, db: 3, q: 1.3 }],
      intake: { amt: 0.16, hz: 1100, q: 1.0 },
      bass: { hz: 100, db: 5 }, thump: { amt: 1, hz: 48, q: 1.4 }, overrunRough: 0, starter: { amt: 1.0, pitch: 1.0 },
      mech: 0.035, crackle: 0, popAmt: 0.45, bang: 0.3, drive: 3.3, gain: 0.38
    }
  },
  {
    id: 'mustang', group: 'na',
    name: 'Ford Mustang GT', tagline: 'Coyote 5.0 L V8 · 7,500 rpm · 10-speed automatic',
    engine: { topMph: 163,
      idle: 700, redline: 7500, limiter: 7500, launch: 2600, revUp: 0.16, revDown: 0.32,
      shift: [1900, 7300], start: { crank: 0.7, crankRpm: 175, flare: 1.9, bark: 0.2, settle: 1.7 },
      gearbox: 'auto', skipShift: true,
      peakKw: 358, mass: 1750, cdA: 0.72, crr: 0.012, maxAccel: 7,
      disp: 5.0,
      ratios: [4.696, 2.985, 2.146, 1.769, 1.52, 1.275, 1.0, 0.854, 0.689, 0.636], final: 3.55, tire: 2.16
    },
    // 1-5-4-8-6-3-7-2, cylinders 1-4 right / 5-8 left. Has an X-pipe.
    firing: crossplane(CROSSPLANE),
    sound: {
      pipes: [{ bank: 0, ms: 5.2, pan: -0.55 }, { bank: 1, ms: 5.6, pan: 0.55 }],
      reflect: -0.23, damp: 0.55, cross: 0.35,
      pulse: { idleMs: 3.6, topMs: 0.8 },
      noise: 0.73, rough: 0.045, trim: 0.05, lope: 0,
      muffler: { closedHz: 800, openHz: 6000 },
      body: [{ hz: 90, db: 5, q: 1.0 }, { hz: 350, db: 3, q: 1.3 }, { hz: 1100, db: 4, q: 1.3 }, { hz: 2600, db: 2, q: 1.3 }],
      intake: { amt: 0.1, hz: 700, q: 1.2 },
      bass: { hz: 95, db: 6.5 }, thump: { amt: 1.2, hz: 40, q: 1.4 }, overrunRough: 0, starter: { amt: 1.0, pitch: 0.9 },
      mech: 0.03, crackle: 0, popAmt: 0.45, bang: 0.3, drive: 3.5, gain: 0.36
    }
  },
  {
    id: 'f812', group: 'na',
    name: 'Ferrari 812 Superfast', tagline: '6.5 L V12 · 8,900 rpm · 7-speed DCT',
    engine: { topMph: 211,
      idle: 900, redline: 8900, limiter: 8900, launch: 3500, revUp: 0.1, revDown: 0.22,
      shift: [3200, 8800], start: { crank: 0.6, crankRpm: 200, flare: 3.2, bark: 0.35, settle: 1.9 },
      gearbox: 'dct',
      peakKw: 588, mass: 1630, cdA: 0.72, crr: 0.011, maxAccel: 9.5,
      disp: 6.5,
      ratios: [3.08, 2.19, 1.63, 1.29, 1.03, 0.84, 0.63], final: 4.18, tire: 2.29
    },
    // 1-7-5-11-3-9-6-12-2-8-4-10, even every 60°, banks alternate.
    firing: alternate(12),
    sound: {
      pipes: [{ bank: 0, ms: 3.6, pan: -0.45 }, { bank: 1, ms: 3.7, pan: 0.45 }],
      reflect: -0.18, damp: 0.5,
      pulse: { idleMs: 2.4, topMs: 0.5 },
      noise: 0.67, rough: 0.015, trim: 0.02, lope: 0,
      muffler: { closedHz: 1300, openHz: 10000 },
      body: [{ hz: 130, db: 3, q: 1.0 }, { hz: 700, db: 4, q: 1.3 }, { hz: 2200, db: 5, q: 1.3 }, { hz: 4000, db: 3, q: 1.3 }],
      intake: { amt: 0.15, hz: 1300, q: 1.2 },
      bass: { hz: 110, db: 4 }, thump: { amt: 1, hz: 52, q: 1.4, sub: 1.4 }, overrunRough: 0, starter: { amt: 0.9, pitch: 1.15 },
      mech: 0.03, crackle: 0, popAmt: 0.45, bang: 0.3, drive: 3.1, gain: 0.35
    }
  },
  {
    id: 'enzo', group: 'na',
    name: 'Ferrari Enzo', tagline: '6.0 L V12 · 8,200 rpm · 6-speed F1 paddle shift',
    engine: { topMph: 218,
      idle: 800, redline: 8000, limiter: 8200, launch: 3800, revUp: 0.1, revDown: 0.22,
      shift: [3300, 7900], start: { crank: 0.8, crankRpm: 185, flare: 2.9, bark: 0.3, settle: 1.9 },
      gearbox: 'seq',
      peakKw: 485, mass: 1365, cdA: 0.7, crr: 0.011, maxAccel: 9,
      disp: 6.0,
      ratios: [3.15, 2.18, 1.57, 1.19, 0.94, 0.76], final: 3.85, tire: 2.18
    },
    firing: alternate(12),
    sound: {
      pipes: [{ bank: 0, ms: 4.4, pan: -0.45 }, { bank: 1, ms: 4.5, pan: 0.45 }],
      reflect: -0.19, damp: 0.47,
      pulse: { idleMs: 2.8, topMs: 0.55 },
      noise: 0.71, rough: 0.02, trim: 0.04, lope: 0,
      muffler: { closedHz: 1000, openHz: 9000 },
      body: [{ hz: 110, db: 5, q: 1.2 }, { hz: 400, db: 3, q: 1.3 }, { hz: 1300, db: 5, q: 1.3 }, { hz: 2600, db: 4, q: 1.3 }],
      intake: { amt: 0.16, hz: 1200, q: 1.2 },
      bass: { hz: 105, db: 4.5 }, thump: { amt: 1, hz: 50, q: 1.4, sub: 1.4 }, overrunRough: 0, starter: { amt: 1.0, pitch: 1.05 },
      mech: 0.03, crackle: 0, popAmt: 0.45, bang: 0.3, drive: 2.9, gain: 0.36
    }
  },
  {
    id: 'svj', group: 'na',
    name: 'Lamborghini Aventador SVJ', tagline: '6.5 L V12 · 8,700 rpm · 7-speed ISR',
    engine: { topMph: 217,
      idle: 950, redline: 8500, limiter: 8700, launch: 3800, revUp: 0.1, revDown: 0.24,
      shift: [3200, 8400], start: { crank: 0.95, crankRpm: 190, flare: 3.4, bark: 0.8, settle: 2.4 },
      gearbox: 'seq',
      peakKw: 566, mass: 1525, cdA: 0.8, crr: 0.011, maxAccel: 10,
      disp: 6.5,
      ratios: [3.91, 2.44, 1.81, 1.46, 1.19, 0.97, 0.84], final: 3.3, tire: 2.25
    },
    firing: alternate(12),
    sound: {
      pipes: [{ bank: 0, ms: 3.0, pan: -0.45 }, { bank: 1, ms: 3.2, pan: 0.45 }],
      reflect: -0.2, damp: 0.6,
      pulse: { idleMs: 2.6, topMs: 0.6 },
      noise: 0.9, rough: 0.04, trim: 0.05, lope: 0,
      muffler: { closedHz: 1600, openHz: 9000 },
      body: [{ hz: 140, db: 2, q: 1.0 }, { hz: 800, db: 4, q: 1.3 }, { hz: 1300, db: 5, q: 1.3 }, { hz: 3200, db: 4, q: 1.3 }],
      intake: { amt: 0.15, hz: 1000, q: 1.2 },
      bass: { hz: 100, db: 5 }, thump: { amt: 1.0, hz: 48, q: 1.4 }, overrunRough: 0, starter: { amt: 1.2, pitch: 1.1 },
      gunshot: { amt: 1.5, flame: 1 },
      mech: 0.03, crackle: 0, popAmt: 0.6, bang: 0.6, bangBurst: [1, 2], bangGapMs: 50, bangCrack: 0.7, drive: 3.2, gain: 0.37
    }
  },

  // ── Turbo & supercharged ──────────────────────────────────────────────
  {
    id: 'civic', group: 'turbo',
    name: 'Honda Civic Type R', tagline: 'FL5 · K20C1 2.0 L turbo four · 7,000 rpm · 6-speed manual',
    engine: { topMph: 169,
      idle: 800, redline: 7000, limiter: 7000, launch: 3200, revUp: 0.14, revDown: 0.26,
      shift: [2600, 6800], start: { crank: 0.5, crankRpm: 230, flare: 1.8, bark: 0.1, settle: 1.3 },
      gearbox: 'manual',
      peakKw: 235, mass: 1430, cdA: 0.68, crr: 0.011, maxAccel: 6,
      disp: 2.0,
      ratios: [3.625, 2.115, 1.529, 1.125, 0.911, 0.734], final: 4.105, tire: 2.02
    },
    // 1-3-4-2 into one collector; three tips in the middle.
    firing: single(4),
    sound: {
      pipes: [{ bank: 0, ms: 3.2, pan: -0.15 }, { bank: 0, ms: 3.35, pan: 0.15 }],
      reflect: -0.2, damp: 0.55,
      pulse: { idleMs: 3.0, topMs: 0.7 },
      noise: 0.79, rough: 0.04, trim: 0.04, lope: 0,
      muffler: { closedHz: 1000, openHz: 7000 },
      body: [{ hz: 140, db: 3, q: 1.0 }, { hz: 600, db: 3, q: 1.3 }, { hz: 1700, db: 4, q: 1.3 }, { hz: 3500, db: 2, q: 1.3 }],
      intake: { amt: 0.16, hz: 1000, q: 1.2 },
      turbo: { amt: 0.9, spoolRpm: 2200, spoolS: 0.55, damp: 0.6, whistle: 0.025, whistleHz: [3000, 9000], hiss: 0.07, bov: 'bov', bovAmt: 0.25 },
      bass: { hz: 130, db: 3.5 }, thump: { amt: 1, hz: 50, q: 1.4, sub: 3 }, overrunRough: 0, starter: { amt: 0.8, pitch: 1.2 },
      mech: 0.035, crackle: 0, popAmt: 0.35, bang: 0.35, bangBurst: [1, 3], bangGapMs: 40, bangCrack: 0.8, drive: 3.5, gain: 0.27
    }
  },
  {
    id: 'supra', group: 'turbo',
    name: 'Toyota Supra (A80)', tagline: '2JZ-GTE 3.0 L twin-turbo six · 6,800 rpm · 6-speed manual',
    engine: { topMph: 177,
      idle: 750, redline: 6800, limiter: 7000, launch: 3000, revUp: 0.18, revDown: 0.3,
      shift: [2500, 6700], start: { crank: 1.0, crankRpm: 210, flare: 1.9, bark: 0.25, settle: 1.6 },
      gearbox: 'manual',
      peakKw: 239, mass: 1560, cdA: 0.7, crr: 0.011, maxAccel: 6,
      disp: 3.0,
      ratios: [3.827, 2.36, 1.685, 1.312, 1.0, 0.793], final: 3.133, tire: 2.02
    },
    // 1-5-3-6-2-4; front three and rear three cylinders feed one turbo each,
    // merging into one exhaust.
    firing: alternate(6),
    sound: {
      pipes: [{ bank: 0, ms: 4.4, pan: -0.2 }, { bank: 1, ms: 4.5, pan: 0.2 }],
      reflect: -0.23, damp: 0.55, cross: 0.8,
      pulse: { idleMs: 3.4, topMs: 0.8 },
      noise: 0.6, rough: 0.03, trim: 0.03, lope: 0,
      muffler: { closedHz: 1000, openHz: 7000 },
      body: [{ hz: 120, db: 4, q: 1.0 }, { hz: 500, db: 3, q: 1.3 }, { hz: 1400, db: 3, q: 1.3 }, { hz: 3200, db: 2, q: 1.3 }],
      intake: { amt: 0.15, hz: 900, q: 1.2 },
      turbo: { amt: 1, spoolRpm: 3500, spoolS: 0.9, damp: 0.55, whistle: 0.035, whistleHz: [2200, 7500], hiss: 0.08, bov: 'flutter', bovAmt: 0.6 },
      bass: { hz: 115, db: 4.5 }, thump: { amt: 1.0, hz: 46, q: 1.4, sub: 1.5 }, overrunRough: 0, starter: { amt: 1.0, pitch: 1.0 },
      mech: 0.04, crackle: 0, popAmt: 0.4, bang: 1.0, bangBurst: [3, 6], bangGapMs: 38, bangCrack: 1.0, drive: 3, gain: 0.39
    }
  },
  {
    id: 'sti', group: 'turbo',
    name: 'Subaru WRX STI', tagline: 'EJ257 2.5 L turbo boxer · unequal-length headers · 6-speed manual',
    engine: { topMph: 159,
      idle: 800, redline: 6700, limiter: 6700, launch: 3500, revUp: 0.16, revDown: 0.3,
      shift: [2600, 6500], start: { crank: 0.8, crankRpm: 200, flare: 2.0, bark: 0.2, settle: 1.6 },
      gearbox: 'manual',
      peakKw: 227, mass: 1530, cdA: 0.72, crr: 0.012, maxAccel: 6,
      disp: 2.5,
      ratios: [3.636, 2.375, 1.761, 1.346, 0.971, 0.756], final: 3.9, tire: 2.06
    },
    // 1-3-2-4 into one Y-pipe; the headers are unequal, so the pulses arrive
    // unevenly: the boxer rumble.
    firing: single(4),
    sound: {
      pipes: [{ bank: 0, ms: 4.8, pan: -0.2 }, { bank: 0, ms: 5.0, pan: 0.2 }],
      reflect: -0.23, damp: 0.57,
      header: [0, 2.6, 0.4, 3.4],
      pulse: { idleMs: 4.0, topMs: 0.9 },
      noise: 0.75, rough: 0.05, trim: 0.08, lope: 0,
      muffler: { closedHz: 800, openHz: 6000 },
      body: [{ hz: 90, db: 5, q: 1.0 }, { hz: 300, db: 4, q: 1.2 }, { hz: 900, db: 3, q: 1.3 }, { hz: 2400, db: 1, q: 1.3 }],
      intake: { amt: 0.12, hz: 700, q: 1.2 },
      turbo: { amt: 0.8, spoolRpm: 3000, spoolS: 0.7, damp: 0.45, whistle: 0.02, whistleHz: [2500, 7000], hiss: 0.06, bov: 'bov', bovAmt: 0.3 },
      bass: { hz: 95, db: 6 }, thump: { amt: 1.2, hz: 42, q: 1.4 }, overrunRough: 0, starter: { amt: 1.0, pitch: 1.05 },
      mech: 0.035, crackle: 0.4, popAmt: 0.4, bang: 0.45, bangBurst: [1, 3], bangGapMs: 45, bangCrack: 0.8, drive: 3.6, gain: 0.25
    }
  },
  {
    id: 'rs3', group: 'turbo',
    name: 'Audi RS 3', tagline: '2.5 L turbo five · 7,000 rpm · 7-speed DCT',
    engine: { topMph: 174,
      idle: 800, redline: 7000, limiter: 7100, launch: 3500, revUp: 0.15, revDown: 0.28,
      shift: [2400, 6900], start: { crank: 0.6, crankRpm: 210, flare: 2.6, bark: 0.4, settle: 1.7 },
      gearbox: 'dct',
      peakKw: 294, mass: 1575, cdA: 0.68, crr: 0.011, maxAccel: 8,
      disp: 2.5,
      ratios: [3.56, 2.53, 1.68, 1.22, 0.95, 0.79, 0.66], final: 4.47, tire: 2.03
    },
    // 1-2-4-5-3, a firing every 144°: the five-cylinder warble.
    firing: single(5),
    sound: {
      pipes: [{ bank: 0, ms: 4.0, pan: -0.45 }, { bank: 0, ms: 4.15, pan: 0.45 }],
      reflect: -0.23, damp: 0.55,
      pulse: { idleMs: 3.2, topMs: 0.7 },
      noise: 0.75, rough: 0.035, trim: 0.05, lope: 0,
      muffler: { closedHz: 900, openHz: 7000 },
      body: [{ hz: 130, db: 4, q: 1.0 }, { hz: 550, db: 4, q: 1.3 }, { hz: 1400, db: 4, q: 1.3 }, { hz: 3000, db: 2, q: 1.3 }],
      intake: { amt: 0.15, hz: 900, q: 1.2 },
      turbo: { amt: 0.8, spoolRpm: 2400, spoolS: 0.6, damp: 0.5, whistle: 0.02, whistleHz: [2800, 7500], hiss: 0.06, bov: 'bov', bovAmt: 0.3 },
      bass: { hz: 110, db: 5 }, thump: { amt: 1, hz: 46, q: 1.4, sub: 1.4 }, overrunRough: 0, starter: { amt: 0.9, pitch: 1.1 },
      mech: 0.035, crackle: 0, popAmt: 0.55, bang: 0.5, bangBurst: [1, 3], bangGapMs: 42, bangCrack: 0.8, drive: 3.5, gain: 0.34
    }
  },
  {
    id: 'm3g80', group: 'turbo',
    name: 'BMW M3 (G80)', tagline: 'S58 3.0 L twin-turbo six · 7,200 rpm · 8-speed automatic',
    engine: { topMph: 180,
      idle: 750, redline: 7200, limiter: 7300, launch: 3000, revUp: 0.18, revDown: 0.3,
      shift: [2200, 7100], start: { crank: 0.6, crankRpm: 200, flare: 2.5, bark: 0.3, settle: 1.7 },
      gearbox: 'auto',
      peakKw: 375, mass: 1730, cdA: 0.7, crr: 0.011, maxAccel: 8,
      disp: 3.0,
      ratios: [5.0, 3.2, 2.143, 1.72, 1.314, 1.0, 0.822, 0.64], final: 3.154, tire: 2.13
    },
    // 1-5-3-6-2-4; front and rear three cylinders on separate turbos.
    firing: alternate(6),
    sound: {
      pipes: [{ bank: 0, ms: 4.5, pan: -0.5 }, { bank: 1, ms: 4.7, pan: 0.5 }],
      reflect: -0.23, damp: 0.55, cross: 0.3,
      pulse: { idleMs: 3.4, topMs: 0.8 },
      noise: 0.75, rough: 0.03, trim: 0.03, lope: 0,
      muffler: { closedHz: 900, openHz: 7000 },
      body: [{ hz: 110, db: 4, q: 1.0 }, { hz: 450, db: 3, q: 1.3 }, { hz: 1300, db: 3, q: 1.3 }, { hz: 2800, db: 2, q: 1.3 }],
      intake: { amt: 0.1, hz: 900, q: 1.2 },
      turbo: { amt: 0.7, spoolRpm: 2200, spoolS: 0.5, damp: 0.55, whistle: 0.012, whistleHz: [3000, 8000], hiss: 0.05, bov: 'bov', bovAmt: 0.25 },
      bass: { hz: 110, db: 5 }, thump: { amt: 1, hz: 46, q: 1.4, sub: 1.4 }, overrunRough: 0, starter: { amt: 0.9, pitch: 1.05 },
      mech: 0.035, crackle: 0.6, popAmt: 0.45, bang: 0.5, bangBurst: [1, 2], bangGapMs: 45, bangCrack: 0.7, drive: 3.5, gain: 0.32
    }
  },
  {
    id: 'gtr', group: 'turbo',
    name: 'Nissan GT-R (R35)', tagline: 'VR38DETT 3.8 L twin-turbo V6 · 7,100 rpm · 6-speed DCT',
    engine: { topMph: 196,
      idle: 750, redline: 7100, limiter: 7100, launch: 3500, revUp: 0.15, revDown: 0.3,
      shift: [2400, 7000], start: { crank: 0.7, crankRpm: 200, flare: 2.2, bark: 0.2, settle: 1.6 },
      gearbox: 'dct',
      peakKw: 421, mass: 1750, cdA: 0.72, crr: 0.011, maxAccel: 9.5,
      disp: 3.8,
      ratios: [4.056, 2.301, 1.595, 1.248, 1.001, 0.796], final: 3.7, tire: 2.22
    },
    // 1-2-3-4-5-6, odd cylinders right, even left: banks alternate every 120°.
    firing: alternate(6),
    sound: {
      pipes: [{ bank: 0, ms: 3.8, pan: -0.5 }, { bank: 1, ms: 4.0, pan: 0.5 }],
      reflect: -0.22, damp: 0.55,
      pulse: { idleMs: 3.0, topMs: 0.7 },
      noise: 0.79, rough: 0.035, trim: 0.04, lope: 0,
      muffler: { closedHz: 900, openHz: 7000 },
      body: [{ hz: 120, db: 3, q: 1.0 }, { hz: 500, db: 3, q: 1.3 }, { hz: 1500, db: 4, q: 1.3 }, { hz: 3200, db: 2, q: 1.3 }],
      intake: { amt: 0.12, hz: 900, q: 1.2 },
      turbo: { amt: 0.85, spoolRpm: 2600, spoolS: 0.5, damp: 0.6, whistle: 0.02, whistleHz: [3000, 8000], hiss: 0.06, bov: 'bov', bovAmt: 0.3 },
      // The rear transaxle's famous whine, rising with road speed.
      gearWhine: { amt: 0.012, ratio: 28 },
      bass: { hz: 105, db: 5 }, thump: { amt: 1, hz: 46, q: 1.4, sub: 1.4 }, overrunRough: 0, starter: { amt: 1.0, pitch: 1.0 },
      mech: 0.035, crackle: 0, popAmt: 0.4, bang: 0.35, bangBurst: [1, 2], bangGapMs: 45, bangCrack: 0.7, drive: 3.5, gain: 0.32
    }
  },
  {
    id: 'urus', group: 'turbo',
    name: 'Lamborghini Urus', tagline: '4.0 L twin-turbo V8 · 6,800 rpm · 8-speed automatic',
    engine: { topMph: 190,
      idle: 650, redline: 6700, limiter: 6800, launch: 3000, revUp: 0.2, revDown: 0.35,
      shift: [1800, 6600], start: { crank: 0.6, crankRpm: 190, flare: 2.9, bark: 0.5, settle: 2.0 },
      gearbox: 'auto',
      peakKw: 478, mass: 2200, cdA: 0.95, crr: 0.012, maxAccel: 8.5,
      disp: 4.0,
      ratios: [4.71, 3.14, 2.11, 1.67, 1.29, 1.0, 0.84, 0.67], final: 3.31, tire: 2.47
    },
    // 1-5-4-8-6-3-7-2, cross-plane, turbos inside the V.
    firing: crossplane(CROSSPLANE),
    sound: {
      pipes: [{ bank: 0, ms: 6.0, pan: -0.55 }, { bank: 1, ms: 7.1, pan: 0.55 }],
      reflect: -0.23, damp: 0.57,
      pulse: { idleMs: 4.8, topMs: 0.8 },
      noise: 1.0, rough: 0.06, trim: 0.09, lope: 0,
      muffler: { closedHz: 650, openHz: 7000 },
      body: [{ hz: 75, db: 7, q: 1.0 }, { hz: 190, db: 3, q: 1.1 }, { hz: 420, db: 3, q: 1.3 }, { hz: 1100, db: 3, q: 1.3 }],
      intake: { amt: 0.15, hz: 750, q: 1.2 },
      turbo: { amt: 0.6, spoolRpm: 1800, spoolS: 0.5, damp: 0.4, whistle: 0.006, whistleHz: [2500, 6500], hiss: 0.04, bov: 'bov', bovAmt: 0.2 },
      bass: { hz: 85, db: 7 }, thump: { amt: 1.3, hz: 38, q: 1.4 }, overrunRough: 0, starter: { amt: 1.0, pitch: 0.95 },
      mech: 0.02, crackle: 0, popAmt: 0.4, bang: 0.45, drive: 3.6, gain: 0.32
    }
  },
  {
    id: 'rs6', group: 'turbo',
    name: 'Audi RS 6 Avant', tagline: 'C8 · 4.0 L twin-turbo V8 · 6,800 rpm · 8-speed automatic',
    engine: { topMph: 190,
      idle: 600, redline: 6700, limiter: 6800, launch: 3000, revUp: 0.2, revDown: 0.35,
      shift: [1800, 6600], start: { crank: 0.6, crankRpm: 190, flare: 2.5, bark: 0.25, settle: 1.8 },
      gearbox: 'auto',
      peakKw: 441, mass: 2075, cdA: 0.8, crr: 0.012, maxAccel: 9,
      disp: 4.0,
      ratios: [4.714, 3.143, 2.106, 1.667, 1.285, 1.0, 0.839, 0.667], final: 3.2, tire: 2.29
    },
    firing: crossplane(CROSSPLANE),
    sound: {
      pipes: [{ bank: 0, ms: 6.2, pan: -0.55 }, { bank: 1, ms: 6.6, pan: 0.55 }],
      reflect: -0.23, damp: 0.53, cross: 0.2,
      pulse: { idleMs: 4.2, topMs: 0.8 },
      noise: 0.79, rough: 0.05, trim: 0.06, lope: 0,
      muffler: { closedHz: 700, openHz: 6500 },
      body: [{ hz: 80, db: 5, q: 1.0 }, { hz: 220, db: 3, q: 1.2 }, { hz: 600, db: 2, q: 1.3 }, { hz: 1500, db: 1, q: 1.3 }],
      intake: { amt: 0.12, hz: 650, q: 1.2 },
      turbo: { amt: 0.65, spoolRpm: 1800, spoolS: 0.5, damp: 0.5, whistle: 0.006, whistleHz: [2500, 6500], hiss: 0.04, bov: 'bov', bovAmt: 0.2 },
      bass: { hz: 90, db: 6.5 }, thump: { amt: 1.2, hz: 40, q: 1.4 }, overrunRough: 0, starter: { amt: 0.9, pitch: 1.0 },
      mech: 0.02, crackle: 0, popAmt: 0.3, bang: 0.3, drive: 3.5, gain: 0.36
    }
  },
  {
    id: 'huracan', group: 'turbo',
    name: 'Lamborghini Huracán Twin Turbo', tagline: 'Underground Racing build · 5.2 L twin-turbo V10 · 8,700 rpm · 7-speed DCT',
    engine: { topMph: 240,
      idle: 1000, redline: 8500, limiter: 8700, launch: 4500, revUp: 0.09, revDown: 0.22,
      shift: [3500, 8400], start: { crank: 0.8, crankRpm: 210, flare: 3.1, bark: 0.85, settle: 2.0 },
      gearbox: 'dct',
      peakKw: 1100, mass: 1480, cdA: 0.75, crr: 0.011, maxAccel: 12,
      disp: 5.2,
      ratios: [3.133, 2.588, 1.958, 1.536, 1.238, 1.022, 0.837], final: 3.267, tire: 2.17
    },
    // 1-6-5-10-2-7-3-8-4-9: even firing every 72°, banks alternating.
    firing: alternate(10),
    sound: {
      // Straight-piped through the turbos. Matched to an onboard recording of
      // the build: a full, mid-heavy roar (most of it 300 Hz-1.6 kHz) that
      // falls away above 2 kHz (kept a bit brighter than the
      // recording so the wail cuts through car speakers); the turbines round off the rasp.
      pipes: [{ bank: 0, ms: 2.9, pan: -0.45 }, { bank: 1, ms: 3.1, pan: 0.45 }],
      reflect: -0.18, damp: 0.6,
      pulse: { idleMs: 2.5, topMs: 0.55 },
      noise: 0.6, rough: 0.04, trim: 0.05, lope: 0,
      muffler: { closedHz: 1500, openHz: 6500 },
      body: [{ hz: 140, db: 2, q: 1.0 }, { hz: 280, db: -5, q: 1.0 }, { hz: 650, db: 4, q: 1.0 }, { hz: 1100, db: 2, q: 1.2 }],
      intake: { amt: 0.4, hz: 800, q: 0.7 },
      toneHz: 6000,
      // Big twin turbos. The whistle sweeps up from about 1.9 kHz as they
      // spool and keeps screaming up top on a hard pull, with bright air
      // rushing over it at high boost; at part throttle the rush sits under
      // the engine. One full, low pssh per lift (none on the quick DCT
      // upshifts): a quick, full pssh centred near 1-1.6 kHz. A faint whine
      // at idle (two of them, ~1.87 and ~1.96 kHz in the recording, beating), and the shaft audibly spinning down after a lift.
      turbo: { amt: 1.2, spoolRpm: 3600, spoolS: 1.0, damp: 0.45, whistle: 0.04, whistleHz: [1870, 9000], whistleTwin: 1.05, whistleFade: 0.55, whistleIdle: 0.12, whistlePulse: 0.4,
        hiss: 0.12, hissHz: 1500, hissQ: 0.8, rush: 0.1,
        psi: 30, bov: 'bov', bovAmt: 0.8, bovHz: [900, 1600], bovQ: 0.5, bovAttack: 0.015, bovDecay: 0.09, bovShift: 0 },
      bass: { hz: 100, db: 5 }, thump: { amt: 1.1, hz: 46, q: 1.4, sub: 1.2 }, overrunRough: 0, starter: { amt: 1.2, pitch: 1.1 },
      gunshot: { amt: 1.4, flame: 0 },
      mech: 0.01, crackle: 0, popAmt: 0.6, bang: 0.75, bangBurst: [1, 3], bangGapMs: 42, bangCrack: 0.7, drive: 3.4, gain: 0.32
    }
  },
  {
    id: 'chiron', group: 'turbo',
    name: 'Bugatti Chiron', tagline: '8.0 L quad-turbo W16 · 6,700 rpm · 7-speed DCT',
    engine: { topMph: 261,
      idle: 800, redline: 6700, limiter: 6800, launch: 3500, revUp: 0.14, revDown: 0.3,
      shift: [2600, 6600], start: { crank: 0.9, crankRpm: 170, flare: 2.4, bark: 0.5, settle: 2.0 },
      gearbox: 'dct',
      peakKw: 1103, mass: 1995, cdA: 0.66, crr: 0.011, maxAccel: 11,
      disp: 8.0,
      ratios: [3.36, 2.27, 1.69, 1.29, 1.03, 0.84, 0.69], final: 3.19, tire: 2.3
    },
    // W16: four banks of four, firing evenly every 45°, into two exhausts.
    firing: alternate(16),
    sound: {
      // Big bore pipes and four turbos: deep, dense and smooth, a muted roar
      // with a heavy wall of boost on top.
      pipes: [{ bank: 0, ms: 4.6, pan: -0.45 }, { bank: 1, ms: 4.8, pan: 0.45 }],
      reflect: -0.2, damp: 0.6,
      pulse: { idleMs: 3.6, topMs: 0.8 },
      noise: 0.8, rough: 0.03, trim: 0.03, lope: 0,
      muffler: { closedHz: 700, openHz: 6000 },
      body: [{ hz: 80, db: 6, q: 1.0 }, { hz: 240, db: 4, q: 1.2 }, { hz: 650, db: 3, q: 1.3 }, { hz: 1600, db: 2, q: 1.3 }],
      intake: { amt: 0.12, hz: 900, q: 1.2 },
      turbo: { amt: 1.1, spoolRpm: 3000, spoolS: 0.8, damp: 0.75, whistle: 0.03, whistleHz: [2200, 6500], hiss: 0.1, bov: 'bov', bovAmt: 0.35 },
      bass: { hz: 85, db: 7 }, thump: { amt: 1.3, hz: 40, q: 1.4, sub: 1.2 }, overrunRough: 0, starter: { amt: 1.1, pitch: 0.95 },
      mech: 0.02, crackle: 0, popAmt: 0.4, bang: 0.5, bangBurst: [1, 2], bangGapMs: 45, bangCrack: 0.6, drive: 3.5, gain: 0.32
    }
  },
  {
    id: 'hellcat', group: 'turbo',
    name: 'Dodge Challenger Hellcat', tagline: '6.2 L supercharged HEMI V8 · 6,200 rpm · 6-speed manual',
    engine: { topMph: 199,
      idle: 700, redline: 6200, limiter: 6400, launch: 2500, revUp: 0.22, revDown: 0.4,
      shift: [2200, 6000], start: { crank: 1.0, crankRpm: 160, flare: 2.3, bark: 0.3, settle: 2.0 },
      gearbox: 'manual',
      peakKw: 527, mass: 2020, cdA: 0.85, crr: 0.012, maxAccel: 7,
      disp: 6.2,
      ratios: [3.01, 2.07, 1.43, 1.00, 0.84, 0.57], final: 3.70, tire: 2.29
    },
    // 1-8-4-3-6-5-7-2, odd cylinders left, even right: cross-plane.
    firing: crossplane([0, 1, 1, 0, 1, 0, 0, 1]),
    sound: {
      pipes: [{ bank: 0, ms: 5.5, pan: -0.55 }, { bank: 1, ms: 5.9, pan: 0.55 }],
      reflect: -0.15, damp: 0.55, cross: 0.1,
      pulse: { idleMs: 4.5, topMs: 0.9 },
      noise: 0.79, rough: 0.05, trim: 0.07, lope: 0,
      muffler: { closedHz: 900, openHz: 7000 },
      body: [{ hz: 60, db: 7, q: 1.0 }, { hz: 220, db: 3, q: 1.0 }, { hz: 800, db: 4, q: 1.3 }, { hz: 1600, db: 4, q: 1.3 }],
      intake: { amt: 0.08, hz: 600, q: 1.2 },
      // Twin-screw blower whine, about 20 × crank speed.
      blower: { amt: 0.16, ratio: 20 },
      bass: { hz: 80, db: 8 }, thump: { amt: 1.4, hz: 36, q: 1.4 }, overrunRough: 0, starter: { amt: 1.2, pitch: 0.8 },
      mech: 0.02, crackle: 0, popAmt: 0.3, bang: 0.2, drive: 3.6, gain: 0.35
    }
  },

  // ── Race ──────────────────────────────────────────────────────────────
  {
    id: 'f1', group: 'race',
    name: 'Formula 1', tagline: '1.6 L turbo V6 hybrid · 12,000 rpm · 8-speed seamless',
    engine: { topMph: 215,
      idle: 4000, redline: 12000, limiter: 12500, launch: 8000, revUp: 0.05, revDown: 0.1,
      shift: [7500, 11800], downRpm: 5500, minGearS: 1.25, start: { crank: 0.8, crankRpm: 900, flare: 1.45, bark: 0.15, settle: 1.0 },
      gearbox: 'seamless',
      peakKw: 750, mass: 800, cdA: 1.1, crr: 0.015, maxAccel: 12,
      disp: 1.6,
      // Real gaps (about 1.2 apart), geared shorter so street speeds reach
      // eighth: 1st to about 64 km/h, 8th to about 230.
      ratios: [3.58, 2.99, 2.49, 2.07, 1.73, 1.44, 1.2, 1.0], final: 4.52, tire: 2.23
    },
    // 90° V6 on three shared crankpins: firings alternate 90° / 150°.
    firing: [[0, 0], [90, 1], [240, 0], [330, 1], [480, 0], [570, 1]],
    sound: {
      pipes: [{ bank: 0, ms: 1.0, pan: -0.1 }, { bank: 1, ms: 1.0, pan: 0.1 }],
      reflect: -0.18, damp: 0.55, cross: 1,
      pulse: { idleMs: 1.6, topMs: 0.35 },
      noise: 0.82, rough: 0.02, trim: 0.03, lope: 0,
      muffler: { closedHz: 1800, openHz: 10000 },
      body: [{ hz: 300, db: 3, q: 1.0 }, { hz: 1100, db: 4, q: 1.3 }, { hz: 2000, db: 4, q: 1.3 }, { hz: 3400, db: 2, q: 1.3 }],
      intake: { amt: 0.16, hz: 1400, q: 1.2 },
      turbo: { amt: 0.5, spoolRpm: 6000, spoolS: 0.3, damp: 0.3, whistle: 0.01, whistleHz: [4000, 10000], hiss: 0.04, bov: 'none', bovAmt: 0.1 },
      // MGU-K whine, geared to the crank.
      blower: { amt: 0.008, ratio: 12.7 },
      bass: { hz: 150, db: 2 }, thump: { amt: 0.6, hz: 70, q: 1.4, sub: 1.2 }, overrunRough: 0, starter: { amt: 0.6, pitch: 1.6 },
      mech: 0.035, crackle: 0, popAmt: 0.25, bang: 0.2, drive: 2.85, gain: 0.44
    }
  },
  // ── Just for fun ──────────────────────────────────────────────────────
  // fitTaller: these may also get taller gearing to fit the Top speed
  // setting (a 40 mph moped would otherwise sit on its limiter).
  {
    id: 'topfuel', group: 'fun',
    name: 'Top Fuel Dragster', tagline: '500 ci nitro Hemi V8 · 11,000 hp · one gear',
    engine: { topMph: 335, fitTaller: true,
      idle: 2300, redline: 8200, limiter: 8400, launch: 6500, revUp: 0.05, revDown: 0.2,
      shift: [6000, 8100], start: { crank: 1.0, crankRpm: 300, flare: 1.7, bark: 1, settle: 1.6 },
      gearbox: 'seq',
      peakKw: 8200, mass: 1050, cdA: 1.0, crr: 0.02, maxAccel: 30,
      disp: 8.2,
      ratios: [1.0], final: 2.62, tire: 2.87
    },
    firing: crossplane(CROSSPLANE),
    sound: {
      // Eight open zoomie pipes, no muffler, nitro: every firing is a blast.
      pipes: [{ bank: 0, ms: 1.2, pan: -0.6 }, { bank: 1, ms: 1.3, pan: 0.6 }],
      reflect: -0.1, damp: 0.7,
      pulse: { idleMs: 5.0, topMs: 1.2 },
      noise: 1.6, rough: 0.12, trim: 0.12, lope: 0,
      muffler: { closedHz: 3000, openHz: 14000 },
      body: [{ hz: 60, db: 8, q: 1.0 }, { hz: 180, db: 5, q: 1.2 }, { hz: 500, db: 4, q: 1.3 }, { hz: 1500, db: 3, q: 1.3 }],
      intake: { amt: 0.25, hz: 700, q: 1.2 },
      blower: { amt: 0.1, ratio: 16 },
      bass: { hz: 70, db: 9 }, thump: { amt: 2.0, hz: 34, q: 1.2, sub: 1.2 }, overrunRough: 0.4, overrunDark: 0.2, starter: { amt: 1.4, pitch: 0.8 },
      mech: 0.03, crackle: 0.8, popAmt: 1.0, bang: 1.0, bangBurst: [2, 4], bangGapMs: 40, bangCrack: 0.8, drive: 5, gain: 0.22
    }
  },
  {
    id: 'groupb', group: 'fun',
    name: 'Group B Rally (Audi S1)', tagline: '2.1 L turbo five · anti-lag · 8,000 rpm · dog box',
    engine: { topMph: 150, fitTaller: true,
      idle: 1000, redline: 8000, limiter: 8100, launch: 4500, revUp: 0.09, revDown: 0.2,
      shift: [3500, 7900], start: { crank: 0.7, crankRpm: 220, flare: 2.6, bark: 1, settle: 1.4 },
      gearbox: 'seq',
      peakKw: 440, mass: 1090, cdA: 0.75, crr: 0.015, maxAccel: 9,
      disp: 2.1,
      ratios: [2.9, 2.0, 1.55, 1.25, 1.03, 0.87], final: 4.5, tire: 2.0
    },
    // 1-2-4-5-3: even firing every 144°, one collector.
    firing: single(5),
    sound: {
      pipes: [{ bank: 0, ms: 3.6, pan: -0.15 }, { bank: 0, ms: 3.75, pan: 0.15 }],
      reflect: -0.2, damp: 0.5,
      pulse: { idleMs: 3.0, topMs: 0.7 },
      noise: 1.0, rough: 0.05, trim: 0.06, lope: 0,
      muffler: { closedHz: 1500, openHz: 9000 },
      body: [{ hz: 130, db: 4, q: 1.0 }, { hz: 550, db: 4, q: 1.3 }, { hz: 1400, db: 5, q: 1.3 }, { hz: 3000, db: 3, q: 1.3 }],
      intake: { amt: 0.16, hz: 1100, q: 1.2 },
      // Anti-lag: no blow-off; the turbo is kept lit off throttle instead.
      turbo: { amt: 1.2, spoolRpm: 3500, spoolS: 0.5, damp: 0.5, whistle: 0.05, whistleHz: [3000, 9000], hiss: 0.12, bov: 'none', bovAmt: 0 },
      antilag: 1,
      bass: { hz: 110, db: 5 }, thump: { amt: 1.1, hz: 46, q: 1.4, sub: 1.4 }, overrunRough: 0.2, overrunDark: 0.3, starter: { amt: 1.0, pitch: 1.1 },
      mech: 0.035, crackle: 0.9, popAmt: 0.9, bang: 0.8, bangBurst: [1, 3], bangGapMs: 45, bangCrack: 0.7, drive: 3.8, gain: 0.23
    }
  },
  {
    id: 'v24', group: 'fun',
    name: 'V24 Hypercar (imaginary)', tagline: '12 L V24 · 12,000 rpm · 8-speed DCT',
    engine: { topMph: 300, fitTaller: true,
      idle: 1400, redline: 12000, limiter: 12300, launch: 5000, revUp: 0.07, revDown: 0.18,
      shift: [4500, 11800], start: { crank: 1.0, crankRpm: 200, flare: 3.2, bark: 0.8, settle: 2.2 },
      gearbox: 'dct',
      peakKw: 1500, mass: 1400, cdA: 0.7, crr: 0.011, maxAccel: 14,
      disp: 12.0,
      ratios: [3.2, 2.4, 1.85, 1.48, 1.22, 1.02, 0.87, 0.76], final: 4.42, tire: 2.25
    },
    firing: alternate(24),
    sound: {
      pipes: [{ bank: 0, ms: 2.6, pan: -0.5 }, { bank: 1, ms: 2.7, pan: 0.5 }],
      reflect: -0.2, damp: 0.6,
      pulse: { idleMs: 2.0, topMs: 0.4 },
      noise: 0.9, rough: 0.04, trim: 0.05, lope: 0,
      muffler: { closedHz: 2000, openHz: 12000 },
      body: [{ hz: 160, db: 3, q: 1.0 }, { hz: 900, db: 4, q: 1.3 }, { hz: 1800, db: 6, q: 1.3 }, { hz: 4200, db: 5, q: 1.3 }],
      intake: { amt: 0.16, hz: 1300, q: 1.2 },
      bass: { hz: 100, db: 5 }, thump: { amt: 1.0, hz: 50, q: 1.4 }, overrunRough: 0, starter: { amt: 1.3, pitch: 1.2 },
      mech: 0.03, crackle: 0, popAmt: 0.6, bang: 0.6, bangBurst: [1, 2], bangGapMs: 45, bangCrack: 0.7, drive: 3.4, gain: 0.34
    }
  },
  {
    id: 'moped', group: 'fun',
    name: '50cc Moped', tagline: 'two-stroke single · 11,000 rpm · CVT · flat out at 40',
    engine: { topMph: 40, fitTaller: true,
      idle: 1800, redline: 11000, limiter: 11500, launch: 6500, revUp: 0.07, revDown: 0.14,
      shift: [7000, 10500], start: { crank: 0.6, crankRpm: 600, flare: 1.6, bark: 0, settle: 0.8 },
      gearbox: 'cvt',
      peakKw: 3.5, mass: 180, cdA: 0.5, crr: 0.02, maxAccel: 2.5,
      disp: 0.05,
      // CVT range (low, high) times the final reduction.
      ratios: [2.6, 0.85], final: 15.7, tire: 1.3
    },
    // Two-stroke: fires every revolution.
    cycle: 360,
    firing: [[0, 0]],
    sound: {
      // Expansion chamber: a strong, nasal resonance (the "ring-ding").
      pipes: [{ bank: 0, ms: 1.3, pan: 0.2 }],
      reflect: -0.5, damp: 0.3,
      pulse: { idleMs: 0.9, topMs: 0.25 },
      noise: 0.9, rough: 0.08, trim: 0, lope: 0,
      muffler: { closedHz: 2500, openHz: 9000 },
      body: [{ hz: 180, db: 2, q: 1.0 }, { hz: 1100, db: 7, q: 2.5 }, { hz: 2600, db: 5, q: 2 }, { hz: 5000, db: 3, q: 1.5 }],
      intake: { amt: 0.2, hz: 1400, q: 1.5 },
      bass: { hz: 120, db: 2 }, thump: { amt: 0.4, hz: 70, q: 1.4, sub: 0.5 }, overrunRough: 0.3, starter: { amt: 0.6, pitch: 1.8 },
      mech: 0.06, crackle: 0.3, popAmt: 0.3, bang: 0.15, drive: 4, gain: 0.49
    }
  },
  {
    id: 'hayabusa', group: 'fun',
    name: 'Suzuki Hayabusa', tagline: 'Gen 3 · 1,340 cc inline four · 11,000 rpm · 6-speed quickshifter',
    engine: { topMph: 186,
      idle: 1150, redline: 11000, limiter: 11000, launch: 5000, revUp: 0.05, revDown: 0.12,
      shift: [5000, 10700], start: { crank: 0.4, crankRpm: 350, flare: 1.7, bark: 0.1, settle: 1.0 },
      gearbox: 'dct',
      // 264 kg wet plus a rider; wheelie-limited off the line.
      peakKw: 140, mass: 345, cdA: 0.35, crr: 0.015, maxAccel: 9,
      disp: 1.34,
      // Primary 1.596 × final 44/17.
      ratios: [2.615, 1.937, 1.526, 1.285, 1.136, 1.043], final: 4.131, tire: 1.95
    },
    // 1-2-4-3, 4-2-1 headers into twin cans, one each side.
    firing: single(4),
    sound: {
      pipes: [{ bank: 0, ms: 2.3, pan: -0.45 }, { bank: 0, ms: 2.45, pan: 0.45 }],
      reflect: -0.3, damp: 0.45,
      pulse: { idleMs: 2.2, topMs: 0.4 },
      noise: 0.8, rough: 0.025, trim: 0.03, lope: 0,
      muffler: { closedHz: 1400, openHz: 9000 },
      body: [{ hz: 160, db: 2, q: 1.0 }, { hz: 750, db: 3, q: 1.3 }, { hz: 2000, db: 4, q: 1.4 }, { hz: 4200, db: 3, q: 1.4 }],
      // Ram-air intake: a big part of the shriek at speed.
      intake: { amt: 0.3, hz: 1300, q: 1.4 },
      bass: { hz: 120, db: 2.5 }, thump: { amt: 0.6, hz: 60, q: 1.4, sub: 0.6 }, overrunRough: 0, starter: { amt: 0.7, pitch: 1.5 },
      mech: 0.05, crackle: 0.4, popAmt: 0.4, bang: 0.25, bangBurst: [1, 2], bangGapMs: 35, bangCrack: 0.8, drive: 3.5, gain: 0.38
    }
  },
  {
    id: 'semi', group: 'fun',
    name: 'Semi Truck', tagline: '15 L Cummins diesel six · jake brake · air brakes · 10-speed',
    engine: { topMph: 80, fitTaller: true,
      idle: 650, redline: 2100, limiter: 2200, launch: 1100, revUp: 0.35, revDown: 0.6,
      shift: [1300, 1900], downRpm: 1000, start: { crank: 1.4, crankRpm: 150, flare: 1.5, bark: 0, settle: 1.8 },
      gearbox: 'manual',
      peakKw: 450, mass: 15000, cdA: 6, crr: 0.007, maxAccel: 1.5,
      disp: 15.0,
      ratios: [12.8, 9.25, 6.76, 4.9, 3.58, 2.61, 1.89, 1.38, 1.0, 0.73], final: 3.55, tire: 3.23
    },
    firing: single(6),
    sound: {
      // Twin vertical stacks; diesel knock; the jake brake on overrun.
      pipes: [{ bank: 0, ms: 9, pan: -0.6 }, { bank: 0, ms: 9.3, pan: 0.6 }],
      reflect: -0.3, damp: 0.5,
      pulse: { idleMs: 6.0, topMs: 2.0 },
      noise: 1.2, rough: 0.05, trim: 0.05, lope: 0,
      muffler: { closedHz: 600, openHz: 4000 },
      body: [{ hz: 70, db: 7, q: 1.0 }, { hz: 180, db: 4, q: 1.2 }, { hz: 450, db: 3, q: 1.3 }, { hz: 1200, db: 3, q: 1.3 }],
      intake: { amt: 0.1, hz: 600, q: 1.2 },
      turbo: { amt: 0.9, spoolRpm: 1100, spoolS: 1.0, damp: 0.8, whistle: 0.03, whistleHz: [1800, 4500], hiss: 0.1, bov: 'none', bovAmt: 0 },
      jake: { amt: 1.2 }, airBrake: 1,
      bass: { hz: 80, db: 8 }, thump: { amt: 1.6, hz: 32, q: 1.4, sub: 1.4 }, overrunRough: 0, overrunDark: 0.1, starter: { amt: 1.6, pitch: 0.7 },
      mech: 0.12, crackle: 0, popAmt: 0.3, bang: 0, drive: 3.6, gain: 0.28
    }
  },
  {
    id: 'tuner', group: 'fun',
    name: 'Skyline GT-R (F&F build)', tagline: 'RB26 straight six · big single turbo · flutter · 6-speed',
    engine: { topMph: 180, fitTaller: true,
      idle: 1000, redline: 8500, limiter: 8600, launch: 4500, revUp: 0.11, revDown: 0.25,
      shift: [3000, 8300], start: { crank: 0.8, crankRpm: 200, flare: 2.4, bark: 0.6, settle: 1.6 },
      gearbox: 'manual',
      peakKw: 560, mass: 1560, cdA: 0.7, crr: 0.012, maxAccel: 9,
      disp: 2.6,
      ratios: [3.827, 2.36, 1.685, 1.312, 1.0, 0.793], final: 3.545, tire: 2.0
    },
    firing: single(6),
    sound: {
      pipes: [{ bank: 0, ms: 4.4, pan: -0.15 }, { bank: 0, ms: 4.5, pan: 0.15 }],
      reflect: -0.23, damp: 0.55, cross: 0.8,
      pulse: { idleMs: 3.3, topMs: 0.75 },
      noise: 0.75, rough: 0.03, trim: 0.03, lope: 0,
      muffler: { closedHz: 1100, openHz: 8000 },
      body: [{ hz: 120, db: 4, q: 1.0 }, { hz: 520, db: 3, q: 1.3 }, { hz: 1400, db: 4, q: 1.3 }, { hz: 3200, db: 2, q: 1.3 }],
      intake: { amt: 0.14, hz: 1000, q: 1.2 },
      // Huge turbo: lag, then a loud whistle; lift-off flutters like a movie.
      turbo: { amt: 1.5, spoolRpm: 4000, spoolS: 0.9, damp: 0.6, whistle: 0.07, whistleHz: [3500, 10000], hiss: 0.15, bov: 'flutter', bovAmt: 0.9 },
      bass: { hz: 115, db: 4.5 }, thump: { amt: 1.0, hz: 46, q: 1.4, sub: 1.5 }, overrunRough: 0, starter: { amt: 1.0, pitch: 1.0 },
      mech: 0.035, crackle: 0, popAmt: 0.5, bang: 0.6, bangBurst: [1, 3], bangGapMs: 40, bangCrack: 0.6, drive: 3.6, gain: 0.32
    }
  },
  {
    id: 'jet', group: 'fun', kind: 'jet',
    name: 'Fighter Jet', tagline: 'afterburning turbofan · 29,000 lbf · full throttle lights the burner',
    engine: { topMph: 1300, mode: 'turbine',
      idle: 2600, redline: 10000, limiter: 10600, launch: 0, revUp: 1.2, revDown: 1.6,
      shift: [0, 0], start: { crank: 2.6, crankRpm: 1200, flare: 1.0, bark: 0, settle: 1.0 },
      gearbox: 'direct',
      peakKw: 20000, mass: 12000, cdA: 2, crr: 0.01, maxAccel: 12,
      ratios: [1], final: 1, tire: 30
    },
    firing: [[0, 0]],
    sound: { gain: 0.422 }
  },
  {
    id: 'steam', group: 'fun', kind: 'steam',
    name: 'Steam Locomotive', tagline: 'two-cylinder express engine · chuffs to your speed · Rev blows the whistle',
    engine: { topMph: 100, mode: 'direct', fitTaller: true,
      // "rpm" here is chuffs per minute: four per turn of the driving wheels.
      idle: 0, redline: 1800, limiter: 2000, launch: 0, revUp: 0.2, revDown: 0.3,
      shift: [0, 0], start: { crank: 0.05, crankRpm: 0, flare: 0, bark: 0, settle: 0.3 },
      gearbox: 'direct',
      peakKw: 6000, mass: 60000, cdA: 10, crr: 0.002, maxAccel: 1.5,
      ratios: [4], final: 1, tire: 6.0
    },
    firing: [[0, 0]],
    sound: { gain: 0.465 }
  },
  {
    id: 'pod', group: 'fun', kind: 'pod',
    name: 'Pod Racer', tagline: 'twin open turbines · buzzing, crackling scream',
    engine: { topMph: 600, mode: 'turbine',
      idle: 3000, redline: 12000, limiter: 12500, launch: 0, revUp: 0.5, revDown: 0.8,
      shift: [0, 0], start: { crank: 1.6, crankRpm: 1500, flare: 1.3, bark: 0, settle: 0.8 },
      gearbox: 'direct',
      peakKw: 5000, mass: 700, cdA: 1, crr: 0.01, maxAccel: 12,
      ratios: [1], final: 1, tire: 30
    },
    firing: [[0, 0]],
    sound: { gain: 0.322 }
  },
  {
    id: 'warp', group: 'fun', kind: 'warp',
    name: 'Starship Warp Drive', tagline: 'deep core hum · hard acceleration jumps to warp',
    engine: { topMph: 9999, mode: 'turbine',
      idle: 1000, redline: 9000, limiter: 9500, launch: 0, revUp: 1.5, revDown: 2.5,
      shift: [0, 0], start: { crank: 1.5, crankRpm: 300, flare: 1.4, bark: 0, settle: 1.5 },
      gearbox: 'direct',
      peakKw: 50000, mass: 5000, cdA: 0.1, crr: 0.001, maxAccel: 12,
      ratios: [1], final: 1, tire: 30
    },
    firing: [[0, 0]],
    sound: { gain: 0.346 }
  }
];
export const GROUPS = [
  { id: 'na', name: 'Naturally aspirated' },
  { id: 'turbo', name: 'Turbo & supercharged' },
  { id: 'race', name: 'Race' },
  { id: 'fun', name: 'Just for fun' }
];

function isObj(x) { return x && typeof x === 'object' && !Array.isArray(x); }

// Deep merge: objects merge key by key, arrays and scalars replace.
export function merge(base, over) {
  if (!isObj(over)) return over == null ? clone(base) : clone(over);
  const out = isObj(base) ? clone(base) : {};
  for (const k of Object.keys(over)) out[k] = isObj(over[k]) && isObj(out[k]) ? merge(out[k], over[k]) : clone(over[k]);
  return out;
}

export function clone(x) { return x === undefined ? undefined : JSON.parse(JSON.stringify(x)); }

// The car as the synthesizer and drivetrain see it, with an optional saved
// tune ({ sound, engine } overrides) on top.
export function resolveCar(car, tune) {
  const sound = merge(merge(DEFAULT_SOUND, car.sound), tune && tune.sound);
  const engine = merge(car.engine, tune && tune.engine);
  engine.cyl = car.firing.length;
  return { id: car.id, name: car.name, kind: car.kind || 'piston', cycle: car.cycle || 720, engine, firing: car.firing, sound };
}

// The parts of `edited` that differ from `base` (arrays compared whole).
export function diff(base, edited) {
  if (isObj(base) && isObj(edited)) {
    const out = {};
    for (const k of Object.keys(edited)) {
      const d = diff(base[k], edited[k]);
      if (d !== undefined) out[k] = d;
    }
    return Object.keys(out).length ? out : undefined;
  }
  return JSON.stringify(base) === JSON.stringify(edited) ? undefined : clone(edited);
}

export function carById(id) { return CARS.find(c => c.id === id) || CARS[0]; }
