//! Vehicle engine audio: the owner's engine sound kit (ported to Rust in
//! `ed_engine_audio`) streamed on the audio thread.
//!
//! - One Bevy audio source, [`EngineMixerAudio`], wraps the kit mixer. Bevy's
//!   rodio stack pulls samples from it on its own thread; the mixer renders
//!   128-frame blocks of every audible voice (README "Driving it from a
//!   game": blocks on the audio thread, controls applied every 32 samples).
//! - Every frame, [`drive_engine_sounds`] turns each vehicle's state (rpm,
//!   gear, throttle, speed from `vehicles.rs`) into the voice's controls and
//!   events with `ed_engine_audio::driver::Driver`, works out distance gain
//!   and pan from the camera, and sends it all through a lock-free queue.
//! - Only the nearest [`MAX_AUDIBLE`] vehicles get a voice (the kit's
//!   performance budget: a voice is about 0.3-1.3% of a core natively, the
//!   whole mix stays under ~8%); the rest are silent until they come closer.
//!
//! Keys: `N` (in a vehicle) cycles its engine sound through the kit.

use crate::vehicles::{Car, Driving, Plane, CAR_FINAL_DRIVE, CAR_WHEEL_R};
use avian3d::prelude::LinearVelocity;
use bevy::audio::{AddAudioSource, ChannelCount, Decodable, SampleRate, Source};
use bevy::prelude::*;
use ed_engine_audio::driver::{DriveInput, Driver, Events, GameEngine};
use ed_engine_audio::mixer::{self, spatialize, Command, DistanceModel, Mixer, MixerHandle, MAX_VOICES};
use ed_engine_audio::{car_index, catalog, Car as KitCar, SAMPLE_RATE};
use std::num::NonZero;
use std::sync::Mutex;
use std::time::Duration;

/// How many vehicles are heard at once (the rest wait their turn by distance).
pub const MAX_AUDIBLE: usize = 6;

/// What kind of sound a vehicle makes. Maps to a kit car by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VehicleSound {
    Car,
    Plane,
    /// TODO(rockets): the kit has no rocket engine. When rocket ships land,
    /// either add a `rocket` generator to `special.rs` (broadband roar +
    /// crackle, like the jet's afterburner without the turbine whine) or
    /// drive the jet with its afterburner held lit. Until then rockets are
    /// silent here.
    #[allow(dead_code)]
    Rocket,
}

impl VehicleSound {
    /// The kit car this vehicle type uses unless the player picks another.
    pub fn default_kit_car(self) -> Option<&'static str> {
        match self {
            // Matched by ear and spectrum to a recording of the real car.
            VehicleSound::Car => Some("huracan"),
            VehicleSound::Plane => Some("jet"),
            VehicleSound::Rocket => None,
        }
    }
    fn game_engine(self) -> GameEngine {
        match self {
            // vehicles.rs: idles at 850, the automatic changes up at 6,300.
            VehicleSound::Car => GameEngine { idle: 850.0, upshift: 6300.0, v_max: 90.0 },
            // The plane tops out around 100-120 m/s; the spool climbs with airspeed.
            _ => GameEngine { idle: 0.0, upshift: 1.0, v_max: 120.0 },
        }
    }
}

/// Kit cars `N` cycles through, by vehicle type.
const CAR_CYCLE: [&str; 12] = ["huracan", "gt3", "mustang", "f458", "lfa", "m3e90", "civic", "supra", "sti", "gtr", "hellcat", "f1"];
const PLANE_CYCLE: [&str; 3] = ["jet", "pod", "warp"];

/// The engine sound of one vehicle.
#[derive(Component)]
pub struct EngineSound {
    pub kind: VehicleSound,
    pub driver: Driver,
    /// Per-vehicle level (1 = as voiced by the kit).
    pub level: f32,
    car_idx: u16,
    events: Events,
}

impl EngineSound {
    /// The default sound for this vehicle type, already started (the
    /// start-up sequence plays as it is spawned).
    pub fn new(kind: VehicleSound) -> Option<Self> {
        let id = kind.default_kit_car()?;
        Self::with_car(kind, id)
    }
    pub fn with_car(kind: VehicleSound, id: &str) -> Option<Self> {
        let idx = car_index(id)?;
        let kit: &'static KitCar = &catalog().cars[idx];
        let mut driver = Driver::new(kit, kind.game_engine());
        driver.start();
        Some(Self { kind, driver, level: 1.0, car_idx: idx as u16, events: Events::default() })
    }
}

/// The mixer, moved onto the audio thread when playback starts.
#[derive(Asset, TypePath)]
pub struct EngineMixerAudio {
    mixer: Mutex<Option<Mixer>>,
}

/// Sample source for rodio: interleaved stereo from the mixer, forever.
pub struct MixerSource {
    mixer: Mixer,
    right: Option<f32>,
}

impl Iterator for MixerSource {
    type Item = f32;
    #[inline]
    fn next(&mut self) -> Option<f32> {
        if let Some(r) = self.right.take() {
            return Some(r);
        }
        let (l, r) = self.mixer.next_frame();
        self.right = Some(r);
        Some(l)
    }
}

impl Source for MixerSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        NonZero::new(2).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        NonZero::new(self.mixer.sample_rate()).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for EngineMixerAudio {
    type Decoder = MixerSource;
    fn decoder(&self) -> MixerSource {
        // Called once when the player entity starts; a second call gets a
        // silent mixer (its queue has no sender).
        let mixer = self.mixer.lock().ok().and_then(|mut m| m.take()).unwrap_or_else(|| mixer::channel(SAMPLE_RATE).1);
        MixerSource { mixer, right: None }
    }
}

/// The game thread's handle on the mixer, and which vehicle owns each voice.
#[derive(Resource)]
pub struct EngineAudio {
    pub handle: MixerHandle,
    owner: [Option<Entity>; MAX_VOICES],
    seed: u32,
    pub distance: DistanceModel,
    /// Master volume 0..1 (the kit default is 0.8).
    pub volume: f32,
    sent_volume: f32,
}

pub struct EngineAudioPlugin;

impl Plugin for EngineAudioPlugin {
    fn build(&self, app: &mut App) {
        let (handle, mixer) = mixer::channel(SAMPLE_RATE);
        app.add_audio_source::<EngineMixerAudio>()
            .insert_resource(EngineAudio {
                handle,
                owner: [None; MAX_VOICES],
                seed: 1,
                distance: DistanceModel::default(),
                volume: 0.8,
                sent_volume: -1.0,
            })
            .insert_resource(PendingMixer(Mutex::new(Some(mixer))))
            .add_systems(Startup, start_output)
            .add_systems(Update, (attach_engine_sounds, cycle_engine_sound, drive_engine_sounds, log_audio_stats).chain());
    }
}

#[derive(Resource)]
struct PendingMixer(Mutex<Option<Mixer>>);

fn start_output(mut commands: Commands, pending: Res<PendingMixer>, mut assets: ResMut<Assets<EngineMixerAudio>>) {
    let Some(mixer) = pending.0.lock().ok().and_then(|mut m| m.take()) else { return };
    let h = assets.add(EngineMixerAudio { mixer: Mutex::new(Some(mixer)) });
    commands.spawn((AudioPlayer(h), PlaybackSettings::ONCE, Name::new("engine audio")));
}

/// Vehicles spawned without a sound get their type's default.
fn attach_engine_sounds(mut commands: Commands, cars: Query<Entity, (Added<Car>, Without<EngineSound>)>, planes: Query<Entity, (Added<Plane>, Without<EngineSound>)>) {
    for e in &cars {
        if let Some(s) = EngineSound::new(VehicleSound::Car) {
            commands.entity(e).insert(s);
        }
    }
    for e in &planes {
        if let Some(s) = EngineSound::new(VehicleSound::Plane) {
            commands.entity(e).insert(s);
        }
    }
}

/// `N` in a vehicle: the next engine sound from the kit.
fn cycle_engine_sound(keys: Res<ButtonInput<KeyCode>>, driving: Res<Driving>, audio: Res<EngineAudio>, mut sounds: Query<&mut EngineSound>) {
    if !keys.just_pressed(KeyCode::KeyN) {
        return;
    }
    let Some(v) = driving.0 else { return };
    let Ok(mut s) = sounds.get_mut(v) else { return };
    let list: &[&str] = match s.kind {
        VehicleSound::Car => &CAR_CYCLE,
        VehicleSound::Plane => &PLANE_CYCLE,
        VehicleSound::Rocket => return,
    };
    let cur = catalog().cars[s.car_idx as usize].id.as_str();
    let next = list[(list.iter().position(|id| *id == cur).map_or(0, |i| i + 1)) % list.len()];
    let Some(idx) = car_index(next) else { return };
    let kit: &'static KitCar = &catalog().cars[idx];
    let game = s.kind.game_engine();
    s.driver = Driver::new(kit, game);
    s.driver.start_running();
    s.car_idx = idx as u16;
    info!("engine sound: {} ({})", kit.name, kit.tagline);
    // The voice (if it has one) switches car without a click.
    for (slot, owner) in audio.owner.iter().enumerate() {
        if *owner == Some(v) {
            audio.handle.send(Command::SetCar { slot: slot as u8, car: idx as u16 });
        }
    }
}

/// Every frame: controls, events and placement for the audible vehicles.
pub fn drive_engine_sounds(
    time: Res<Time>,
    mut audio: ResMut<EngineAudio>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    mut vehicles: Query<(Entity, &GlobalTransform, &LinearVelocity, &mut EngineSound, Option<&Car>, Option<&Plane>)>,
) {
    let dt = time.delta_secs_f64();
    if dt <= 0.0 {
        return;
    }
    if audio.volume != audio.sent_volume {
        let v = audio.volume;
        audio.handle.send(Command::Master { volume: v });
        audio.sent_volume = v;
    }
    let Ok(cam) = cams.single() else { return };
    let (lis, right) = (cam.translation().to_array(), cam.right().as_vec3().to_array());

    // 1. Advance every vehicle's driver (even unheard ones, so their engine
    //    state is right when they come into earshot) and rank by loudness.
    let mut ranked: Vec<(Entity, f32)> = Vec::new();
    for (e, gt, lv, mut s, car, plane) in &mut vehicles {
        let speed = lv.0.length() as f64;
        let inp = if let Some(c) = car {
            let wheel_rps = speed / (std::f64::consts::TAU * CAR_WHEEL_R as f64);
            DriveInput {
                dt,
                throttle: c.throttle as f64,
                speed,
                engine_rpm: c.rpm as f64,
                gear: c.gear as i32,
                shaft_rpm: wheel_rps * 60.0 * CAR_FINAL_DRIVE as f64,
                horn: 0.0,
            }
        } else if let Some(p) = plane {
            DriveInput { dt, throttle: p.throttle as f64, speed, ..Default::default() }
        } else {
            DriveInput { dt, speed, ..Default::default() }
        };
        let s = &mut *s;
        let _ = s.driver.update(&inp, &mut s.events);
        let sp = spatialize(gt.translation().to_array(), lis, right, &audio.distance);
        ranked.push((e, sp.gain * s.level));
    }
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    ranked.retain(|r| r.1 > 1e-3);
    ranked.truncate(MAX_AUDIBLE);

    // 2. Voices for vehicles that dropped out are released; newcomers start.
    for slot in 0..MAX_VOICES {
        if let Some(o) = audio.owner[slot]
            && !ranked.iter().any(|r| r.0 == o)
        {
            audio.handle.send(Command::Release { slot: slot as u8 });
            audio.owner[slot] = None;
        }
    }
    for &(e, _) in &ranked {
        if audio.owner.contains(&Some(e)) {
            continue;
        }
        let Some(slot) = audio.owner.iter().position(Option::is_none) else { break };
        let Ok((_, _, _, s, _, _)) = vehicles.get(e) else { continue };
        audio.seed = audio.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let seed = audio.seed | 1;
        audio.handle.send(Command::Start { slot: slot as u8, car: s.car_idx, seed });
        audio.owner[slot] = Some(e);
    }

    // 3. Controls, events and placement for every voice.
    for slot in 0..MAX_VOICES {
        let Some(e) = audio.owner[slot] else { continue };
        let Ok((_, gt, _, s, _, _)) = vehicles.get(e) else { continue };
        let slot8 = slot as u8;
        for (event, amt) in s.events.iter() {
            audio.handle.send(Command::Event { slot: slot8, event, amt: amt as f32 });
        }
        for &amt in s.events.clicks() {
            audio.handle.send(Command::Click { slot: slot8, amt: amt as f32 });
        }
        let mut sp = spatialize(gt.translation().to_array(), lis, right, &audio.distance);
        sp.gain *= s.level;
        audio.handle.send(Command::Update { slot: slot8, params: s.driver_params(), spatial: sp });
    }
}

impl EngineSound {
    /// The controls the driver produced this frame.
    fn driver_params(&self) -> ed_engine_audio::Params {
        self.driver.params()
    }
}

/// Every 10 s: how hard the audio thread works (blocks of 128 frames have
/// 2.67 ms each at 48 kHz).
fn log_audio_stats(
    audio: Res<EngineAudio>,
    driving: Res<Driving>,
    sounds: Query<&EngineSound>,
    mut last: Local<Option<(std::time::Instant, u64, u64)>>,
) {
    use std::sync::atomic::Ordering::Relaxed;
    let now = std::time::Instant::now();
    let prev = *last.get_or_insert((now, 0, 0));
    if now.duration_since(prev.0).as_secs_f64() < 10.0 {
        return;
    }
    let last = last.as_mut().unwrap();
    let sh = &audio.handle.shared;
    let (blocks, total) = (sh.blocks.load(Relaxed), sh.total_ns.load(Relaxed));
    let (db, dt) = (blocks - last.1, total - last.2);
    if db == 0 {
        debug!("engine audio: no output stream (no audio device?)");
    } else {
        let voices = audio.owner.iter().filter(|o| o.is_some()).count();
        if let Some(s) = driving.0.and_then(|v| sounds.get(v).ok()) {
            let p = s.driver.params();
            info!("engine audio: driving {} at {:.0} rpm, load {:.2}, overrun {:.2}", s.driver.car().name, p.rpm, p.load, p.overrun);
        }
        info!(
            "engine audio: {voices} voices, {:.0} us/block avg ({:.1}% of the block budget), worst {:.0} us",
            dt as f64 / db as f64 / 1000.0,
            dt as f64 / db as f64 / (128.0 / SAMPLE_RATE as f64 * 1e9) * 100.0,
            sh.worst_ns.load(Relaxed) as f64 / 1000.0
        );
    }
    *last = (now, blocks, total);
}
