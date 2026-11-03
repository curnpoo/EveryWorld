//! Several engine voices on the audio thread (README "Several cars at once").
//!
//! The game thread holds a [`MixerHandle`] and sends [`Command`]s (controls
//! about 60 times a second, events as they happen) through a bounded
//! lock-free channel. The audio thread owns the [`Mixer`]: every 128-frame
//! block it drains the channel, renders each audible voice, applies its
//! distance gain, pan and air absorption (smoothed across the block, so
//! nothing zippers), sums them, and runs the kit's output stage
//! (`audio.js`): compressor → master volume → soft clipper.
//!
//! Real-time rules: all voices are built when the mixer is made; nothing on
//! the audio path allocates, locks or frees. A new vehicle reuses a slot
//! (`EngineVoice::reset`, allocation-free).

use crate::cars::catalog;
use crate::voice::{EngineVoice, Event, Params};
use crossbeam_channel::{Receiver, Sender, TrySendError};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

/// Voices the mixer can host at once. The game keeps the nearest few audible
/// (each voice is ~0.3-1.3% of a desktop core natively; see NOTES.md).
pub const MAX_VOICES: usize = 8;
/// Frames per audio block (the JS worklet's 128; controls apply every 32).
pub const FRAMES: usize = 128;
const QUEUE: usize = 1024;

/// Where a voice sits relative to the listener.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spatial {
    /// Linear gain (distance attenuation × per-vehicle level).
    pub gain: f32,
    /// -1 (left) .. 1 (right).
    pub pan: f32,
    /// 1 keeps the voice's own stereo (pipes left and right, up close);
    /// 0 is a mono point source (far away).
    pub width: f32,
    /// Air absorption: one-pole low-pass cutoff in Hz (20 kHz is open).
    pub air_hz: f32,
}

impl Spatial {
    pub const CENTER: Spatial = Spatial { gain: 1.0, pan: 0.0, width: 1.0, air_hz: 20000.0 };
    pub const SILENT: Spatial = Spatial { gain: 0.0, pan: 0.0, width: 1.0, air_hz: 20000.0 };
}

/// Distance model for [`spatialize`].
#[derive(Clone, Copy, Debug)]
pub struct DistanceModel {
    /// Full level inside this distance (m).
    pub ref_m: f32,
    /// Inaudible (gain 0) beyond this (m).
    pub max_m: f32,
    /// Inverse-distance rolloff factor.
    pub rolloff: f32,
}

impl Default for DistanceModel {
    fn default() -> Self {
        Self { ref_m: 10.0, max_m: 900.0, rolloff: 1.0 }
    }
}

/// Distance gain, pan and air for an emitter, given the listener's position
/// and its right-hand unit vector (any consistent 3D frame).
pub fn spatialize(emitter: [f32; 3], listener: [f32; 3], right: [f32; 3], m: &DistanceModel) -> Spatial {
    let d = [emitter[0] - listener[0], emitter[1] - listener[1], emitter[2] - listener[2]];
    let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if dist >= m.max_m {
        return Spatial { gain: 0.0, ..Spatial::CENTER };
    }
    // Inverse distance (clamped), faded to 0 over the last 25% of max_m.
    let inv = m.ref_m / (m.ref_m + m.rolloff * (dist - m.ref_m).max(0.0));
    let edge = ((m.max_m - dist) / (0.25 * m.max_m)).clamp(0.0, 1.0);
    let pan = if dist > 1e-3 { ((d[0] * right[0] + d[1] * right[1] + d[2] * right[2]) / dist).clamp(-1.0, 1.0) } else { 0.0 };
    // Up close the full width and a centred image; the pan grows with
    // distance, but never hard to one side (both ears hear a passing car).
    let near = (1.0 - (dist - m.ref_m * 0.5) / (m.ref_m * 2.0)).clamp(0.0, 1.0);
    Spatial {
        gain: inv * edge,
        pan: 0.75 * pan * (1.0 - 0.6 * near),
        width: 0.3 + 0.7 * near,
        air_hz: (20000.0 * (-dist / 350.0).exp()).max(1200.0),
    }
}

/// What the game thread sends.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    /// A new vehicle in `slot`: reset the voice to `car` (catalog index).
    Start { slot: u8, car: u16, seed: u32 },
    /// Change the car on a running voice (the crank keeps turning).
    SetCar { slot: u8, car: u16 },
    /// Controls and placement, ~60 Hz.
    Update { slot: u8, params: Params, spatial: Spatial },
    Event { slot: u8, event: Event, amt: f32 },
    Click { slot: u8, amt: f32 },
    /// The vehicle is gone: fade the voice out and free the slot.
    Release { slot: u8 },
    /// Master volume 0..1 (the kit maps it as v^1.6 × 1.4).
    Master { volume: f32 },
    /// character (stock 0 .. open pipes 1), crackle 0..1, for every voice.
    User { character: f32, crackle: f32 },
}

/// Read back from the audio thread (lock-free).
pub struct Shared {
    boost: [AtomicU32; MAX_VOICES],
    /// Blocks rendered, and the slowest block's render time (ns).
    pub blocks: AtomicU64,
    pub worst_ns: AtomicU64,
    pub total_ns: AtomicU64,
}

impl Shared {
    fn new() -> Self {
        Self {
            boost: std::array::from_fn(|_| AtomicU32::new(0)),
            blocks: AtomicU64::new(0),
            worst_ns: AtomicU64::new(0),
            total_ns: AtomicU64::new(0),
        }
    }
    /// Turbo boost 0..1 of a voice, for a gauge.
    pub fn boost(&self, slot: usize) -> f32 {
        f32::from_bits(self.boost[slot.min(MAX_VOICES - 1)].load(Ordering::Relaxed))
    }
}

/// The game thread's side.
#[derive(Clone)]
pub struct MixerHandle {
    tx: Sender<Command>,
    pub shared: Arc<Shared>,
}

impl MixerHandle {
    /// Never blocks. A full queue drops the command (controls are re-sent
    /// every frame, so a drop is harmless).
    pub fn send(&self, c: Command) -> bool {
        match self.tx.try_send(c) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => false,
        }
    }
}

/// A mixer and the handle that drives it.
pub fn channel(sample_rate: u32) -> (MixerHandle, Mixer) {
    let (tx, rx) = crossbeam_channel::bounded(QUEUE);
    let shared = Arc::new(Shared::new());
    (MixerHandle { tx, shared: shared.clone() }, Mixer::new(sample_rate, rx, shared))
}

struct Slot {
    voice: EngineVoice,
    active: bool,
    releasing: bool,
    /// Release fade 1 → 0.
    fade: f32,
    cur: Spatial,
    target: Spatial,
    air_l: f32,
    air_r: f32,
}

/// The audio thread's side: hosts the voices and renders the mix.
pub struct Mixer {
    sr: f32,
    rx: Receiver<Command>,
    shared: Arc<Shared>,
    slots: Vec<Slot>,
    scratch_l: [f32; FRAMES],
    scratch_r: [f32; FRAMES],
    mix_l: [f32; FRAMES],
    mix_r: [f32; FRAMES],
    master: f32,
    master_target: f32,
    comp_env: f32,
    comp_att: f32,
    comp_rel: f32,
    /// Leftover frames of the last block (for odd-sized pulls).
    pending: usize,
    pending_pos: usize,
    character: f32,
    crackle: f32,
}

impl Mixer {
    pub fn new(sample_rate: u32, rx: Receiver<Command>, shared: Arc<Shared>) -> Self {
        let sr = sample_rate as f32;
        let first = &catalog().cars[0];
        let slots = (0..MAX_VOICES)
            .map(|i| Slot {
                voice: EngineVoice::new(sr as f64, first, 1 + i as u32),
                active: false,
                releasing: false,
                fade: 1.0,
                cur: Spatial::SILENT,
                target: Spatial::SILENT,
                air_l: 0.0,
                air_r: 0.0,
            })
            .collect();
        Self {
            sr,
            rx,
            shared,
            slots,
            scratch_l: [0.0; FRAMES],
            scratch_r: [0.0; FRAMES],
            mix_l: [0.0; FRAMES],
            mix_r: [0.0; FRAMES],
            master: 0.0,
            master_target: master_gain(0.8),
            comp_env: 0.0,
            comp_att: 1.0 - (-1.0 / (0.004 * sr)).exp(),
            comp_rel: 1.0 - (-1.0 / (0.15 * sr)).exp(),
            pending: 0,
            pending_pos: 0,
            character: 0.75,
            crackle: 0.5,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sr as u32
    }

    /// Voices currently rendering.
    pub fn active(&self) -> usize {
        self.slots.iter().filter(|s| s.active).count()
    }

    fn apply(&mut self, c: Command) {
        let cars = &catalog().cars;
        let slot = |s: u8| (s as usize) < MAX_VOICES;
        match c {
            Command::Start { slot: s, car, seed } if slot(s) => {
                if let Some(car) = cars.get(car as usize) {
                    let sl = &mut self.slots[s as usize];
                    sl.voice.reset(car, seed);
                    sl.voice.set_user(self.character as f64, self.crackle as f64);
                    sl.active = true;
                    sl.releasing = false;
                    sl.fade = 1.0;
                    sl.cur = Spatial::SILENT;
                    sl.target = Spatial::SILENT;
                    sl.air_l = 0.0;
                    sl.air_r = 0.0;
                }
            }
            Command::SetCar { slot: s, car } if slot(s) => {
                if let Some(car) = cars.get(car as usize) {
                    self.slots[s as usize].voice.configure(car);
                }
            }
            Command::Update { slot: s, params, spatial } if slot(s) => {
                let sl = &mut self.slots[s as usize];
                sl.voice.set_params(&params);
                sl.target = spatial;
            }
            Command::Event { slot: s, event, amt } if slot(s) => self.slots[s as usize].voice.event(event, amt as f64),
            Command::Click { slot: s, amt } if slot(s) => self.slots[s as usize].voice.click(amt as f64),
            Command::Release { slot: s } if slot(s) => self.slots[s as usize].releasing = true,
            Command::Master { volume } => self.master_target = master_gain(volume),
            Command::User { character, crackle } => {
                self.character = character;
                self.crackle = crackle;
                for sl in &mut self.slots {
                    sl.voice.set_user(character as f64, crackle as f64);
                }
            }
            _ => {}
        }
    }

    /// One block of FRAMES into mix_l / mix_r.
    fn block(&mut self) {
        let t0 = std::time::Instant::now();
        while let Ok(c) = self.rx.try_recv() {
            self.apply(c);
        }
        self.mix_l.fill(0.0);
        self.mix_r.fill(0.0);
        let inv_n = 1.0 / FRAMES as f32;
        let fade_step = FRAMES as f32 / (0.08 * self.sr);
        for (i, sl) in self.slots.iter_mut().enumerate() {
            if !sl.active {
                continue;
            }
            let (from, mut to) = (sl.cur, sl.target);
            let f0 = sl.fade;
            if sl.releasing {
                sl.fade = (sl.fade - fade_step).max(0.0);
            }
            to.gain *= sl.fade;
            let g_from = from.gain * f0;
            if g_from < 1e-5 && to.gain < 1e-5 {
                // Inaudible: skip the DSP (the voice catches up when heard).
                sl.cur = sl.target;
                if sl.releasing && sl.fade <= 0.0 {
                    sl.active = false;
                }
                continue;
            }
            sl.voice.render(&mut self.scratch_l, &mut self.scratch_r);
            self.shared.boost[i].store((sl.voice.boost as f32).to_bits(), Ordering::Relaxed);
            let a_from = air_coef(from.air_hz, self.sr);
            let a_to = air_coef(to.air_hz, self.sr);
            for k in 0..FRAMES {
                let x = k as f32 * inv_n;
                let g = g_from + (to.gain - g_from) * x;
                let pan = from.pan + (to.pan - from.pan) * x;
                let w = from.width + (to.width - from.width) * x;
                let a = a_from + (a_to - a_from) * x;
                let (l, r) = (self.scratch_l[k], self.scratch_r[k]);
                let m = 0.5 * (l + r);
                let (l, r) = (w * l + (1.0 - w) * m, w * r + (1.0 - w) * m);
                sl.air_l += a * (l - sl.air_l);
                sl.air_r += a * (r - sl.air_r);
                // Equal-power pan, unity at the centre.
                let ang = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
                let (pl, pr) = (ang.cos() * std::f32::consts::SQRT_2, ang.sin() * std::f32::consts::SQRT_2);
                self.mix_l[k] += sl.air_l * g * pl;
                self.mix_r[k] += sl.air_r * g * pr;
            }
            sl.cur = sl.target;
            if sl.releasing && sl.fade <= 0.0 {
                sl.active = false;
            }
        }
        // Output stage (audio.js): compressor (threshold -14 dB, knee 10 dB,
        // ratio 4, attack 4 ms, release 150 ms) → master volume → soft clip.
        for k in 0..FRAMES {
            let (l, r) = (self.mix_l[k], self.mix_r[k]);
            let peak = l.abs().max(r.abs());
            let c = if peak > self.comp_env { self.comp_att } else { self.comp_rel };
            self.comp_env += c * (peak - self.comp_env);
            let g = compressor_gain(self.comp_env);
            self.master += (self.master_target - self.master) * 0.0005;
            let gm = g * self.master;
            self.mix_l[k] = soft_clip(l * gm);
            self.mix_r[k] = soft_clip(r * gm);
        }
        let ns = t0.elapsed().as_nanos() as u64;
        self.shared.blocks.fetch_add(1, Ordering::Relaxed);
        self.shared.total_ns.fetch_add(ns, Ordering::Relaxed);
        self.shared.worst_ns.fetch_max(ns, Ordering::Relaxed);
    }

    /// Fill two channels.
    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let n = out_l.len().min(out_r.len());
        let mut i = 0;
        while i < n {
            if self.pending == 0 {
                self.block();
                self.pending = FRAMES;
                self.pending_pos = 0;
            }
            let m = self.pending.min(n - i);
            out_l[i..i + m].copy_from_slice(&self.mix_l[self.pending_pos..self.pending_pos + m]);
            out_r[i..i + m].copy_from_slice(&self.mix_r[self.pending_pos..self.pending_pos + m]);
            self.pending -= m;
            self.pending_pos += m;
            i += m;
        }
    }

    /// The next stereo frame (for sample-pulling hosts such as rodio).
    #[inline]
    pub fn next_frame(&mut self) -> (f32, f32) {
        if self.pending == 0 {
            self.block();
            self.pending = FRAMES;
            self.pending_pos = 0;
        }
        let k = self.pending_pos;
        self.pending -= 1;
        self.pending_pos += 1;
        (self.mix_l[k], self.mix_r[k])
    }
}

/// The kit's volume curve: v^1.6 × 1.4.
pub fn master_gain(volume: f32) -> f32 {
    volume.clamp(0.0, 1.0).powf(1.6) * 1.4
}

fn air_coef(hz: f32, sr: f32) -> f32 {
    1.0 - (-std::f32::consts::TAU * hz.clamp(200.0, 22000.0) / sr).exp()
}

/// Static gain of the compressor for an envelope level (soft knee).
fn compressor_gain(env: f32) -> f32 {
    const THRESH: f32 = -14.0;
    const KNEE: f32 = 10.0;
    const RATIO: f32 = 4.0;
    if env < 1e-6 {
        return 1.0;
    }
    let x = 20.0 * env.log10();
    let over = x - THRESH;
    let red = if over <= -KNEE / 2.0 {
        0.0
    } else if over >= KNEE / 2.0 {
        over * (1.0 - 1.0 / RATIO)
    } else {
        let o = over + KNEE / 2.0;
        (1.0 - 1.0 / RATIO) * o * o / (2.0 * KNEE)
    };
    10f32.powf(-red / 20.0)
}

/// audio.js clipCurve: linear to 0.85, then a tanh shoulder to 0.98.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    let a = x.abs().min(1.0);
    let y = if a < 0.85 { a } else { 0.85 + 0.13 * ((a - 0.85) / 0.13).tanh() };
    y.copysign(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cars::car_index;

    fn params(rpm: f64, load: f64) -> Params {
        Params { rpm, load, ..Params::default() }
    }

    #[test]
    fn spatial_falls_off_and_pans() {
        let m = DistanceModel::default();
        let near = spatialize([5.0, 0.0, 0.0], [0.0; 3], [1.0, 0.0, 0.0], &m);
        let far = spatialize([200.0, 0.0, 0.0], [0.0; 3], [1.0, 0.0, 0.0], &m);
        let left = spatialize([-200.0, 0.0, 0.0], [0.0; 3], [1.0, 0.0, 0.0], &m);
        let gone = spatialize([2000.0, 0.0, 0.0], [0.0; 3], [1.0, 0.0, 0.0], &m);
        assert!((near.gain - 1.0).abs() < 1e-6);
        assert!(far.gain < 0.1 && far.gain > 0.0);
        assert!(far.pan > 0.7 && left.pan < -0.7);
        assert!(far.air_hz < near.air_hz);
        assert_eq!(gone.gain, 0.0);
    }

    #[test]
    fn mixes_voices_and_releases() {
        let (h, mut mx) = channel(48_000);
        let gt3 = car_index("gt3").unwrap() as u16;
        let jet = car_index("jet").unwrap() as u16;
        h.send(Command::Start { slot: 0, car: gt3, seed: 7 });
        h.send(Command::Start { slot: 1, car: jet, seed: 8 });
        h.send(Command::Update { slot: 0, params: params(3000.0, 0.6), spatial: Spatial::CENTER });
        h.send(Command::Update { slot: 1, params: params(6000.0, 0.5), spatial: Spatial { gain: 0.3, pan: 0.8, width: 0.0, air_hz: 4000.0 } });
        let (mut l, mut r) = (vec![0.0; 48_000], vec![0.0; 48_000]);
        mx.render(&mut l, &mut r);
        assert_eq!(mx.active(), 2);
        let rms = crate::analysis::rms(&l[24_000..]);
        assert!(rms > 0.02, "audible: {rms}");
        assert!(l.iter().chain(&r).all(|v| v.is_finite() && v.abs() <= 1.0));
        h.send(Command::Release { slot: 0 });
        h.send(Command::Release { slot: 1 });
        mx.render(&mut l, &mut r);
        assert_eq!(mx.active(), 0);
        assert!(crate::analysis::rms(&l[24_000..]) < 1e-4);
        assert!(h.shared.blocks.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn soft_clip_matches_kit_curve() {
        assert_eq!(soft_clip(0.5), 0.5);
        assert!(soft_clip(1.0) < 0.98 && soft_clip(1.0) > 0.95);
        assert_eq!(soft_clip(-0.5), -0.5);
    }
}
