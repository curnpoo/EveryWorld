//! Bevy glue for the procedural soundscape (`ed_audio`, `docs/plans/sound.md`).
//!
//! Every frame this reads the world around the listener (player state,
//! weather, sun, the tile's `SoundMap` and surface, nearby trees, the vehicle
//! being driven), fills one `ed_audio::Params` snapshot and turns state
//! changes into one-shot `Event`s (footsteps, landings, ignitions, stage
//! kicks, chute crack, tree whooshes, thunder...). The synth runs on the
//! audio thread; nothing here allocates per frame beyond tiny Vecs.

use std::num::NonZero;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use avian3d::prelude::{AngularVelocity, LinearVelocity};
use bevy::audio::{AddAudioSource, AudioSink, AudioSinkPlayback, ChannelCount, Decodable, SampleRate, Source, SpatialAudioSink, SpatialListener, SpatialScale, Volume};
use bevy::prelude::*;
use ed_audio::params::{self as ap, blast, fauna, rain_on, tyre, veg};
use ed_audio::{Event as Ev, FaunaCtx, Params, Scape, ScapeHandle, Surface};
use ed_world::biome::Biome;
use ed_world::soundmap::{SoundMap, src};
use ed_world::surface::{Cover, Ground, LAYERS};

use crate::foliage::{TreeChunks, Wind};
use crate::ground_scatter::GroundCover;
use crate::origin::WorldOrigin;
use crate::player::{MoveMode, Player};
use crate::sky::{Clock, CurrentWeather};
use crate::vehicles::{Car, Driving, Plane};
use crate::net::{RemoteAvatar, RemotePlayers, RemoteProxy};
use std::collections::HashMap;

/// A fine tile's sound map, on the tile entity next to its `GroundCover`.
#[derive(Component)]
pub struct TileSound(pub Arc<SoundMap>);

pub struct SoundscapePlugin;

impl Plugin for SoundscapePlugin {
    fn build(&self, app: &mut App) {
        let (handle, scape) = ed_audio::channel();
        app.add_audio_source::<ScapeAudio>()
            .insert_resource(Soundscape { handle })
            .insert_resource(PendingScape(Mutex::new(Some(scape))))
            .insert_resource(RemoteSoundscapes::default())
            .add_systems(Startup, start_output)
            .add_systems(Update, gate_soundscape)
            .add_systems(PostUpdate, (drive_soundscape, drive_remote_soundscapes));
    }
}

#[derive(Resource)]
pub struct Soundscape {
    pub handle: ScapeHandle,
}

#[derive(Resource)]
struct PendingScape(Mutex<Option<Scape>>);

#[derive(Resource, Default)]
struct RemoteSoundscapes(HashMap<u64, RemoteSoundscape>);

struct RemoteSoundscape {
    handle: ScapeHandle,
    entity: Entity,
    mode: Option<MoveMode>,
    boots_on: bool,
    wingsuit: bool,
    step_clock: f32,
    step_left: bool,
    boost_stage: u8,
    cutoff_hz: Arc<AtomicU32>,
}

#[derive(Asset, TypePath)]
struct ScapeAudio {
    scape: Mutex<Option<Scape>>,
    cutoff_hz: Option<Arc<AtomicU32>>,
}

struct ScapeSource {
    scape: Scape,
    cutoff_hz: Option<Arc<AtomicU32>>,
    lowpass: [f32; 2],
    channel: usize,
}

impl Iterator for ScapeSource {
    type Item = f32;
    #[inline]
    fn next(&mut self) -> Option<f32> {
        let sample = self.scape.next_sample();
        let Some(cutoff) = &self.cutoff_hz else {
            return Some(sample);
        };
        let hz = f32::from_bits(cutoff.load(Ordering::Relaxed)).clamp(300.0, 20_000.0);
        let alpha = 1.0 - (-std::f32::consts::TAU * hz / ed_audio::dsp::SR).exp();
        let channel = self.channel;
        self.channel ^= 1;
        self.lowpass[channel] += alpha * (sample - self.lowpass[channel]);
        Some(self.lowpass[channel])
    }
}

impl Source for ScapeSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        NonZero::new(2).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        NonZero::new(ed_audio::dsp::SR as u32).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for ScapeAudio {
    type Decoder = ScapeSource;
    fn decoder(&self) -> ScapeSource {
        // A second decoder gets a silent, disconnected scape.
        let scape = self.scape.lock().ok().and_then(|mut s| s.take()).unwrap_or_else(|| ed_audio::channel().1);
        ScapeSource {
            scape,
            cutoff_hz: self.cutoff_hz.clone(),
            lowpass: [0.0; 2],
            channel: 0,
        }
    }
}

fn start_output(mut commands: Commands, pending: Res<PendingScape>, mut assets: ResMut<Assets<ScapeAudio>>) {
    let Some(scape) = pending.0.lock().ok().and_then(|mut s| s.take()) else { return };
    let h = assets.add(ScapeAudio { scape: Mutex::new(Some(scape)), cutoff_hz: None });
    commands.spawn((AudioPlayer(h), PlaybackSettings::ONCE.paused(), WorldSound, Name::new("soundscape")));
}

#[derive(Component)]
struct WorldSound;

#[derive(Component)]
struct RemoteWorldSound;

/// Each avatar gets a small, independently driven soundscape. Bevy positions
/// each emitter in 3D; we apply the game's explicit hearing range and flyby
/// loudness so distant clients remain silent and fast passes read clearly.
fn drive_remote_soundscapes(
    mut commands: Commands,
    remote: Res<RemotePlayers>,
    mut voices: ResMut<RemoteSoundscapes>,
    mut assets: ResMut<Assets<ScapeAudio>>,
    avatars: Query<(&Transform, &RemoteProxy, &RemoteAvatar)>,
    cameras: Query<(Entity, &GlobalTransform), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    mut sinks: Query<&mut SpatialAudioSink, With<RemoteWorldSound>>,
    mix: Option<Res<crate::settings::AudioSettings>>,
    game_state: Res<State<crate::menu::GameState>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let listener = cameras.iter().next();
    if let Some((camera, _)) = listener {
        commands.entity(camera).insert(SpatialListener::default());
    }
    let listener_pos = listener.map(|(_, t)| t.translation()).unwrap_or(Vec3::ZERO);
    let sound_level = mix.as_ref().map(|m| m.level() * m.air.max(m.body).clamp(0.0, 1.0)).unwrap_or(1.0);
    let quiet = crate::menu_music::music_playing(game_state.get());

    for (transform, proxy, avatar) in &avatars {
        let id = avatar.id;
        let entry = voices.0.entry(id).or_insert_with(|| {
            let (handle, scape) = ed_audio::channel();
            let cutoff_hz = Arc::new(AtomicU32::new(18_000.0f32.to_bits()));
            let asset = assets.add(ScapeAudio {
                scape: Mutex::new(Some(scape)),
                cutoff_hz: Some(cutoff_hz.clone()),
            });
            let entity = commands.spawn((
                AudioPlayer(asset),
                PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Loop,
                    spatial: true,
                    // Compress Rodio's inverse-square curve; the game applies
                    // its own shared falloff to every remote sound below.
                    spatial_scale: Some(SpatialScale::new(0.1)),
                    ..PlaybackSettings::default()
                },
                Transform::from_translation(transform.translation),
                RemoteWorldSound,
                Name::new(format!("remote audio {id}")),
            )).id();
            RemoteSoundscape {
                handle, entity, mode: None, boots_on: false, wingsuit: false,
                step_clock: 0.0, step_left: true, boost_stage: 0, cutoff_hz,
            }
        });

        let state = &proxy.state;
        let speed = state.vel.length();
        let distance = transform.translation.distance(listener_pos);
        let flyby = (speed / 90.0).clamp(0.0, 1.0);
        let thrust = state.throttle.max(if state.afterburner {
            1.5
        } else if state.mode == MoveMode::RocketShoes {
            0.8
        } else {
            0.0
        });
        let boots = thrust > 0.01 && !matches!(state.mode, MoveMode::Walk | MoveMode::Parachute);
        // Counter Rodio's 1/r² sink loss, then apply one shared game curve to
        // all remote sounds: full near 10 ft, quieter with distance, still
        // audible for rockets and movement far away.
        let scaled_distance = distance * 0.1;
        let rodio_compensation = (scaled_distance * scaled_distance).max(1.0);
        let distance_gain = if boots {
            0.35 + 0.65 * (-distance / 40.0).exp()
        } else {
            0.02 + 0.98 * (-distance / 30.0).exp()
        };
        let range = (1.0 - distance / 3_000.0).clamp(0.0, 1.0);
        let gain = sound_level * range * distance_gain * rodio_compensation * (1.0 + 0.3 * flyby);
        let muffle = ((distance - 10.0) / 140.0).clamp(0.0, 1.0);
        let cutoff = 18_000.0 + (1_800.0 - 18_000.0) * muffle * muffle * (3.0 - 2.0 * muffle);
        entry.cutoff_hz.store(cutoff.to_bits(), Ordering::Relaxed);
        commands.entity(entry.entity).insert(Transform::from_translation(transform.translation));
        if let Ok(mut sink) = sinks.get_mut(entry.entity) {
            sink.set_volume(Volume::Linear(gain));
            let toward_listener = (listener_pos - transform.translation).normalize_or_zero();
            let radial = state.vel.dot(toward_listener).clamp(-100.0, 100.0);
            // Use the true radial speed and a wider pitch span so an approach
            // and pass-by are audible at game scale.
            sink.set_speed((343.0 / (343.0 - radial)).clamp(0.7, 1.4));
            if quiet { sink.pause(); } else { sink.play(); }
        }

        let mode = state.mode;
        let falling = matches!(mode, MoveMode::Fall | MoveMode::Skydive);
        let mut params = Params::default();
        params.master = 0.55;
        params.mix = mix.as_ref().map(|m| m.scape_mix()).unwrap_or([1.0; 4]);
        params.wingsuit = (falling && state.wingsuit_on) as u8 as f32;
        params.freefall = (falling && !state.wingsuit_on) as u8 as f32;
        params.canopy = (mode == MoveMode::Parachute) as u8 as f32;
        params.wind = (speed / 70.0).clamp(0.0, 1.0);
        params.gust = (flyby * 0.55).clamp(0.0, 1.0);
        if boots {
            let exhaust = (thrust * 1.1).clamp(0.8, 4.0);
            params.boots = [exhaust; 2];
            params.afterburner = if state.afterburner { 1.2 } else { 0.0 };
        }
        entry.handle.set(params);

        if entry.mode != Some(mode) {
            if entry.mode == Some(MoveMode::Walk) && mode != MoveMode::Walk {
                entry.handle.event(Ev::Launch { surface: Surface::Paving, charge: state.jump_charge.max(0.3) });
            } else if entry.mode.is_some_and(|m| m != MoveMode::Walk) && mode == MoveMode::Walk {
                entry.handle.event(Ev::Land { surface: Surface::Paving, impact: speed.min(24.0) });
            }
            if mode == MoveMode::Parachute {
                entry.handle.event(Ev::ChuteOpen);
            }
            let chirp = match mode { MoveMode::RocketShoes => Some(1), MoveMode::Skydive => Some(2), MoveMode::Parachute => Some(3), _ => None };
            if let Some(mode) = chirp { entry.handle.event(Ev::ModeChirp { mode }); }
            entry.mode = Some(mode);
        }
        if state.wingsuit_on != entry.wingsuit {
            entry.handle.event(if state.wingsuit_on { Ev::SuitOpen } else { Ev::SuitClose });
            entry.wingsuit = state.wingsuit_on;
        }
        if boots && !entry.boots_on {
            entry.handle.event(Ev::Ignite { boot: 0 });
            entry.handle.event(Ev::Ignite { boot: 1 });
        }
        if boots && state.afterburner {
            let stage = (1.0 + speed / 30.0).floor().clamp(1.0, 3.0) as u8;
            if stage > entry.boost_stage { entry.handle.event(Ev::BoostKick { stage }); }
            entry.boost_stage = stage;
        }
        if !boots && entry.boots_on {
            entry.handle.event(Ev::Cutoff { burn_s: 0.5 });
            entry.boost_stage = 0;
        }
        entry.boots_on = boots;

        entry.step_clock -= dt;
        if mode == MoveMode::Walk && speed > 0.8 && entry.step_clock <= 0.0 {
            entry.handle.event(Ev::Footstep { surface: Surface::Paving, force: (speed / 4.0).clamp(0.2, 0.9), pan: if entry.step_left { -0.3 } else { 0.3 }, sprint: speed > 4.0 });
            entry.step_left = !entry.step_left;
            entry.step_clock = (0.52 - speed * 0.045).clamp(0.27, 0.52);
        }
    }

    let stale: Vec<u64> = voices.0.keys().copied().filter(|id| !remote.players.contains_key(id)).collect();
    for id in stale {
        if let Some(voice) = voices.0.remove(&id) { commands.entity(voice.entity).despawn(); }
    }
}

/// Nothing from the world plays on the main menu (the menu music has the speakers).
fn gate_soundscape(state: Res<State<crate::menu::GameState>>, mut sinks: Query<&mut AudioSink, With<WorldSound>>) {
    let quiet = crate::menu_music::music_playing(state.get());
    for mut s in &mut sinks {
        if quiet != s.is_paused() {
            if quiet { s.pause() } else { s.play() }
        }
    }
}

/// What changed since last frame (events are edges).
#[derive(Default)]
struct Edges {
    init: bool,
    mode: Option<MoveMode>,
    grounded: bool,
    planted: [bool; 2],
    boots_on: bool,
    burn_s: f32,
    /// Seconds continuously airborne and falling; alerts wait for a real fall.
    fall_s: f32,
    /// Highest AGL of the current fall: the altimeter is for jumps from altitude.
    fall_top: f32,
    stage: usize,
    wingsuit: bool,
    charge_full: bool,
    prev_vel: Vec3,
    rng: u32,
    /// Recently whooshed trees (rounded position, seconds left).
    whooshed: Vec<(IVec3, f32)>,
    thump_cool: [f32; 4],
    driving: bool,
}

impl Edges {
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// What the listener stands on, from the fine tile under them.
#[derive(Default, Clone, Copy)]
struct Ground1 {
    found: bool,
    splat: [f32; LAYERS],
    cover: Option<Cover>,
    canopy: f32,
    biome: Option<Biome>,
}

fn ss(e0: f32, e1: f32, x: f32) -> f32 {
    ed_audio::dsp::smoothstep(e0, e1, x)
}

/// 1 at the source, 0 by `far` metres (smooth).
fn near(d: f32, close: f32, far: f32) -> f32 {
    1.0 - ss(close, far, d)
}

fn dominant(w: &[f32; LAYERS]) -> Ground {
    let (i, _) = w.iter().enumerate().fold((0, -1.0), |a, (i, &x)| if x > a.1 { (i, x) } else { a });
    Ground::ALL[i]
}

fn surface_of(g: &Ground1) -> Surface {
    if !g.found {
        return Surface::Grass;
    }
    if g.cover == Some(Cover::Water) {
        return Surface::Water;
    }
    Surface::from_ground(dominant(&g.splat))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive_soundscape(
    time: Res<Time>,
    audio: Res<Soundscape>,
    mix: Option<Res<crate::settings::AudioSettings>>,
    origin: Res<WorldOrigin>,
    clock: Option<Res<Clock>>,
    weather: Option<Res<CurrentWeather>>,
    wind: Option<Res<Wind>>,
    driving: Option<Res<Driving>>,
    players: Query<(&GlobalTransform, &LinearVelocity, Option<&AngularVelocity>, &Player)>,
    cars: Query<(&GlobalTransform, &LinearVelocity, &Car)>,
    planes: Query<(&GlobalTransform, &LinearVelocity, &Plane)>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    tiles: Query<(&GroundCover, Option<&TileSound>, &GlobalTransform)>,
    tree_sets: Query<(&TreeChunks, &GlobalTransform)>,
    mut e: Local<Edges>,
) {
    let dt = time.delta_secs().clamp(1e-4, 0.1);
    let Ok(cam) = cams.single() else { return };
    let player = players.single().ok();
    let vehicle = driving.as_ref().and_then(|d| d.0);
    let car = vehicle.and_then(|v| cars.get(v).ok());
    let plane = vehicle.and_then(|v| planes.get(v).ok());
    // The listener rides the vehicle when driving, the player otherwise, the camera in --fly.
    let (pos, vel) = if let Some((t, v, _)) = car {
        (t.translation(), v.0)
    } else if let Some((t, v, _)) = plane {
        (t.translation(), v.0)
    } else if let Some((t, v, _, _)) = player {
        (t.translation(), v.0)
    } else {
        (cam.translation(), Vec3::ZERO)
    };
    if !e.init {
        e.init = true;
        e.rng = 0x9E37_79B9;
        e.prev_vel = vel;
    }
    let up = origin.up_at(pos);
    let geo = origin.to_geodetic(pos);
    let air = ed_flight::atmosphere(geo.alt_m.max(0.0));
    let density = if geo.alt_m > 120_000.0 { 0.0 } else { air.density as f32 };
    let speed = vel.length();
    let wx = weather.as_ref().and_then(|w| w.0.clone());

    let mut p = Params { density, ..Params::default() };
    p.q = ap::dynamic_pressure(density, speed);
    p.mach = speed / air.speed_of_sound.max(1.0) as f32;
    if speed > 1.0 {
        p.flow_side = (vel / speed).dot(cam.right().as_vec3()).clamp(-1.0, 1.0);
    }
    p.reentry = ss(2.0e6, 1.0e8, density * speed * speed * speed);
    p.grid_hz = ap::grid_hz(geo.lat_deg, geo.lon_deg);

    // ---- the ground and the tile sound map under the listener
    let mut g1 = Ground1::default();
    let mut dist = [ed_world::soundmap::FAR_M; src::N];
    let (mut bcover, mut bh, mut rsteep) = (0.0f32, 0.0f32, 0.0f32);
    for (gc, ts, gt) in &tiles {
        let s = &gc.0;
        let local = gt.affine().inverse().transform_point3(pos);
        let (u, v) = s.approx_uv(local);
        let inside = (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v);
        if inside {
            let h = s.grid.sample(u, v);
            g1 = Ground1 {
                found: true,
                splat: s.surface.splat(u, v, h, 0.0),
                cover: Some(s.surface.cover_at(u, v)),
                canopy: s.surface.canopy_share(u, v),
                biome: Some(s.surface.biome),
            };
        }
        if let Some(TileSound(sm)) = ts {
            // Sample the nearest point of every nearby tile and add the gap,
            // so a motorway just across the tile border still counts.
            let width = s.chunk_size_m() * ed_world::ground_scatter::CHUNKS as f32;
            let (cu, cv) = (u.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
            let gap = ((u - cu).hypot(v - cv) as f32) * width;
            if gap > 2_000.0 {
                continue;
            }
            let smp = sm.sample(cu as f32, cv as f32);
            for k in 0..src::N {
                dist[k] = dist[k].min(smp.dist[k] + gap);
            }
            if inside {
                (bcover, bh, rsteep) = (smp.bldg_cover, smp.bldg_h, smp.river_steep);
            }
        }
    }
    // Height above the ground nearly erases ground-level sources.
    let agl = player.map(|(_, _, _, pl)| pl.agl).unwrap_or(0.0).max(0.0);
    let lift = |d: f32| d.hypot(agl);
    let biome = g1.biome.unwrap_or(Biome::TemperateForest);
    let w = |k: Ground| g1.splat[k as usize];
    let cover = g1.cover.unwrap_or(Cover::Natural);
    let urban_cover = matches!(cover, Cover::Residential | Cover::Commercial | Cover::Industrial) as u8 as f32;
    let urban = (bcover * 2.5).max(0.6 * urban_cover).min(1.0);

    // ---- weather, sun, temperature
    let (wind_ms, gust_ms) = match (&wx, &wind) {
        (Some(x), _) => (x.wind_speed_ms, (x.wind_gusts_ms - x.wind_speed_ms).max(0.0)),
        (None, Some(w)) => (w.strength * 8.0, w.strength * 3.0),
        _ => (3.0, 1.5),
    };
    // Wind grows with height above the ground (log profile, capped).
    let shear = 1.0 + 0.25 * (1.0 + agl / 10.0).ln().min(2.0);
    p.wind = wind_ms * shear;
    p.gust = gust_ms * shear;
    let code = wx.as_ref().map(|x| x.weather_code).unwrap_or(0);
    let temp = wx.as_ref().map(|x| x.temperature_c).unwrap_or(15.0) - 0.0065 * agl;
    p.temp_c = temp;
    let rain = wx.as_ref().map(|x| x.rain_mm).unwrap_or(0.0);
    p.drizzle = (matches!(code, 51..=57) as u8 as f32).max(ss(0.0, 0.3, rain) * (rain < 0.5) as u8 as f32);
    p.rain = rain * (rain >= 0.3) as u8 as f32;
    p.hail = matches!(code, 96 | 99) as u8 as f32;
    p.fog = matches!(code, 45 | 48) as u8 as f32;
    let snowing = wx.as_ref().map(|x| x.snowfall_cm).unwrap_or(0.0) > 0.0;
    p.snow = w(Ground::Snow).max(if snowing { 0.8 } else { 0.0 });
    let sun_el = clock
        .as_ref()
        .map(|c| ed_geo::sun::sun_position(geo.lat_deg, geo.lon_deg, c.unix_s).elevation_deg as f32)
        .unwrap_or(30.0);
    let daylight = ss(-6.0, 6.0, sun_el);
    let twilight = (-((sun_el - 1.0) / 7.0).powi(2)).exp();

    // ---- water and city
    let (d_ocean, d_lake, d_river, d_fall) = (lift(dist[src::OCEAN]), lift(dist[src::LAKE]), lift(dist[src::RIVER]), lift(dist[src::WATERFALL]));
    p.surf = near(d_ocean, 15.0, 900.0);
    p.surf_rocky = (w(Ground::Rock) + w(Ground::Gravel)).min(1.0).max(if biome == Biome::Alpine { 0.6 } else { 0.0 });
    p.lake = near(d_lake, 3.0, 120.0);
    p.river = near(d_river, 5.0, 260.0);
    p.river_steep = rsteep.max(near(d_fall, 50.0, 400.0));
    p.waterfall = near(d_fall, 20.0, 800.0);
    p.traffic_far = near(lift(dist[src::MOTORWAY]), 20.0, 1_500.0);
    p.traffic_near = near(lift(dist[src::ARTERIAL]), 8.0, 350.0);
    p.traffic_minor = near(lift(dist[src::MINOR]), 4.0, 120.0) * (0.3 + 0.7 * urban);
    p.rail = near(lift(dist[src::RAIL]), 20.0, 900.0);
    p.crowd = (cover == Cover::Commercial) as u8 as f32 * (0.3 + 0.7 * daylight) * near(agl, 5.0, 60.0);
    p.industry = (cover == Cover::Industrial) as u8 as f32 * near(agl, 10.0, 120.0);
    p.hum = (0.4 * urban + 0.6 * p.industry).min(1.0) * near(agl, 3.0, 30.0);
    p.sodium = urban * (1.0 - daylight) * near(agl, 3.0, 20.0);
    p.echo = ss(0.2, 0.5, bcover) * ss(6.0, 20.0, bh) * near(agl, bh.max(10.0), bh.max(10.0) * 2.0);
    p.echo_delay_s = 2.0 * (30.0 * (1.0 - bcover) + 10.0) / ap::SOUND_SPEED;

    // ---- plants: trees near the listener by voice, plus grass and reeds underfoot
    let mut vw = [0.0f32; veg::N];
    let mut close: Vec<(Vec3, f32)> = Vec::new();
    let whoosh_r = (6.0 + speed * 0.15).min(30.0);
    for (tc, gt) in &tree_sets {
        let to_local = gt.affine().inverse();
        let local = to_local.transform_point3(pos);
        for t in tc.trees_near(local, 45.0) {
            let d = t.pos.distance(local);
            vw[ed_audio::veg_class(t.kind)] += t.scale.min(3.0) / (1.0 + d / 8.0) * 0.15;
            if speed > 12.0 && d < whoosh_r {
                close.push((gt.transform_point(t.pos), t.scale));
            }
        }
    }
    vw[veg::GRASS] += (w(Ground::Grass) + w(Ground::DryGrass) + 0.5 * w(Ground::Farmland)) * 0.6;
    vw[veg::REEDS] += (cover == Cover::Wetland) as u8 as f32 * 0.8 + p.lake * 0.3 * (1.0 - urban);
    if cover == Cover::Shrub {
        vw[veg::SHRUB] += 0.5;
    }
    let ground_fade = near(agl, 8.0, 80.0);
    for (o, x) in p.veg.iter_mut().zip(vw) {
        *o = x.min(1.0) * ground_fade;
    }

    // ---- rain lands on...
    let leaves = (g1.canopy + vw.iter().take(veg::SHRUB).sum::<f32>()).min(1.0);
    let wet = (p.lake + p.surf + p.river).min(1.0);
    p.rain_on[rain_on::LEAVES] = leaves;
    p.rain_on[rain_on::PAVING] = (w(Ground::Paving) + 0.5 * urban).min(1.0);
    p.rain_on[rain_on::WATER] = wet;
    p.rain_on[rain_on::ROOF] = if car.is_some() || plane.is_some() { 1.0 } else { 0.0 };
    p.exposed = if car.is_some() || plane.is_some() { 0.0 } else { 1.0 };

    // ---- animals
    let ctx = FaunaCtx {
        daylight,
        twilight,
        temp_c: temp,
        near_water: p.lake.max(p.river),
        wetland: (cover == Cover::Wetland) as u8 as f32,
        coastal: p.surf,
        pasture: matches!(biome, Biome::Alpine | Biome::Grassland | Biome::TemperateForest) as u8 as f32
            * (w(Ground::Grass) + w(Ground::Farmland)).min(1.0)
            * (1.0 - urban),
        frozen_lake: p.lake * (temp < -2.0) as u8 as f32,
        urban,
    };
    let fm = ed_audio::fauna_mix(biome, ctx);
    for (o, x) in p.fauna.iter_mut().zip(fm) {
        *o = x * ground_fade;
    }
    // Cities still have pigeons and sparrows by day.
    p.fauna[fauna::BIRDS] = p.fauna[fauna::BIRDS].max(0.25 * urban * daylight * ground_fade);

    // ---- the player's body and boots
    let surface = surface_of(&g1);
    if let Some((_, _, ang, pl)) = player.filter(|_| vehicle.is_none()) {
        let falling = matches!(pl.mode, MoveMode::Fall | MoveMode::Skydive);
        p.wingsuit = (falling && pl.wingsuit_on) as u8 as f32;
        p.freefall = (falling && !pl.wingsuit_on) as u8 as f32;
        p.canopy = (pl.mode == MoveMode::Parachute) as u8 as f32;
        p.tuck = pl.fall_tuck;
        p.spread = pl.fall_spread;
        p.wing_load = (0.6 * pl.wing_turn.abs() + 0.6 * pl.wing_arch.max(0.0)).min(1.0);
        p.roll_rate = ang.map(|a| a.0.dot(vel.normalize_or_zero())).unwrap_or(0.0);
        let boots_on = pl.thrust_held && pl.throttle > 0.01;
        if boots_on {
            let wob = pl.leg_steer.x * 0.25;
            p.boots = [pl.throttle * (1.0 + wob), pl.throttle * (1.0 - wob)];
            p.afterburner = if pl.afterburner { ss(0.0, 1.5, pl.boost_time) } else { 0.0 };
            p.leg_steer = pl.leg_steer.x.clamp(-1.0, 1.0);
            p.blast = near(pl.agl, 1.0, 14.0) * pl.throttle.min(1.5) / 1.5;
            let mut bk = [0.0f32; blast::N];
            bk[blast::DUST] = w(Ground::Sand) + w(Ground::Dirt) + w(Ground::DryGrass) + w(Ground::Farmland);
            bk[blast::GRASS] = w(Ground::Grass) + w(Ground::Moss) + w(Ground::ForestFloor) + w(Ground::Mud);
            bk[blast::SNOW] = w(Ground::Snow);
            bk[blast::PAVING] = w(Ground::Paving) + w(Ground::Rock) + w(Ground::Gravel);
            if surface == Surface::Water || wet > 0.9 {
                bk = [0.0; blast::N];
                bk[blast::SPRAY] = 1.0;
            }
            let sum: f32 = bk.iter().sum::<f32>().max(1e-3);
            p.blast_kind = bk.map(|x| x / sum);
        }
        p.jump_charge = pl.jump_charge;
        let limb = pl.rot_err.length() * 1.5 + if pl.grounded { vel.reject_from(up).length() * 0.08 } else { 0.0 };
        p.servo = (limb + 0.5 * pl.jump_charge).min(1.0);
        // Skydiving instruments.
        // Gated on a sustained fall so spawning in (dropped from the sky,
        // or settling onto the ground) doesn't set them off.
        let vz_down = -vel.dot(up);
        let airborne = !pl.grounded && vz_down > 8.0;
        e.fall_s = if airborne { e.fall_s + dt } else { 0.0 };
        e.fall_top = if airborne { e.fall_top.max(pl.agl) } else { pl.agl };
        let real_fall = e.fall_s > 4.0;
        if falling && real_fall && e.fall_top > 1_800.0 && pl.mode == MoveMode::Skydive {
            p.altimeter = ap::altimeter_rate(pl.agl, vz_down);
        }
        if real_fall && !matches!(pl.mode, MoveMode::Walk | MoveMode::Parachute) {
            p.gpw = ap::gpw_level(pl.agl, vz_down);
        }

        // ---- events
        let mode_changed = e.mode.is_some_and(|m| m != pl.mode);
        if mode_changed {
            let chirp = match pl.mode {
                MoveMode::Walk | MoveMode::Fall => None,
                MoveMode::RocketShoes => Some(1),
                MoveMode::Skydive => Some(2),
                MoveMode::Parachute => Some(3),
            };
            if let Some(m) = chirp {
                audio.handle.event(Ev::ModeChirp { mode: m });
            }
            if pl.mode == MoveMode::Parachute {
                audio.handle.event(Ev::ChuteOpen);
            }
        }
        e.mode = Some(pl.mode);
        if pl.wingsuit_on != e.wingsuit {
            audio.handle.event(if pl.wingsuit_on { Ev::SuitOpen } else { Ev::SuitClose });
            if pl.wingsuit_on {
                audio.handle.event(Ev::ModeChirp { mode: 4 });
            }
            e.wingsuit = pl.wingsuit_on;
        }
        // Footsteps: a foot that plants while walking.
        if pl.grounded && pl.mode == MoveMode::Walk {
            let hs = vel.reject_from(up).length();
            for i in 0..2 {
                if pl.foot_planted[i] && !e.planted[i] && hs > 0.3 {
                    let force = if pl.crouch { 0.2 } else { (0.35 + hs / 9.0).min(1.0) };
                    audio.handle.event(Ev::Footstep { surface, force, pan: if i == 0 { -0.3 } else { 0.3 }, sprint: pl.sprint });
                }
            }
        }
        e.planted = pl.foot_planted;
        if pl.grounded && !e.grounded {
            let impact = (-e.prev_vel.dot(up)).max(0.0);
            if impact > 1.5 {
                audio.handle.event(Ev::Land { surface, impact });
            }
            if pl.mode == MoveMode::Walk && (surface == Surface::Water || wet > 0.95) && impact > 3.0 {
                audio.handle.event(Ev::Splash { force: (impact / 15.0).min(1.0) });
            }
        }
        if !pl.grounded && e.grounded && vel.dot(up) > 2.0 {
            audio.handle.event(Ev::Launch { surface, charge: pl.jump_charge.max(0.3) });
        }
        e.grounded = pl.grounded;
        let full = pl.jump_charge > 0.98;
        if full && !e.charge_full {
            audio.handle.event(Ev::ChargeFull);
        }
        e.charge_full = full;
        if boots_on && !e.boots_on {
            audio.handle.event(Ev::Ignite { boot: 0 });
            audio.handle.event(Ev::Ignite { boot: 1 });
            e.burn_s = 0.0;
        }
        if boots_on {
            e.burn_s += dt;
            if pl.boost_stage > e.stage {
                audio.handle.event(Ev::BoostKick { stage: pl.boost_stage as u8 });
            }
        }
        if !boots_on && e.boots_on {
            audio.handle.event(Ev::Cutoff { burn_s: e.burn_s });
        }
        e.boots_on = boots_on;
        e.stage = pl.boost_stage;
    }

    // ---- vehicles
    if let Some((_, _, c)) = car {
        p.tyre_speed = speed;
        let road = near(dist[src::MINOR].min(dist[src::ARTERIAL]).min(dist[src::MOTORWAY]), 4.0, 12.0);
        let asphalt = (w(Ground::Paving) + road).min(1.0);
        let gravel = (w(Ground::Gravel) + w(Ground::Rock)) * (1.0 - asphalt);
        let dirt = (1.0 - asphalt - gravel).max(0.0);
        p.tyre_on[tyre::ASPHALT] = asphalt;
        p.tyre_on[tyre::GRAVEL] = gravel;
        p.tyre_on[tyre::DIRT] = dirt;
        p.skid = c.slip.iter().copied().fold(0.0, f32::max) * ss(2.0, 6.0, speed);
        p.cabin_wind = ss(10.0, 50.0, speed);
        for i in 0..4 {
            e.thump_cool[i] -= dt;
            if c.comp_vel[i] > 1.8 && e.thump_cool[i] <= 0.0 {
                audio.handle.event(Ev::Thump { strength: ((c.comp_vel[i] - 1.8) / 3.0).min(1.0), pan: if i % 2 == 0 { -0.5 } else { 0.5 } });
                e.thump_cool[i] = 0.25;
            }
        }
    }
    if let Some((_, _, pl)) = plane {
        // The horn starts near the critical angle (~15 deg) and is full past 25.
        p.stall = ss(12.0, 25.0, pl.aoa.to_degrees()) * ss(10.0, 20.0, pl.airspeed);
        p.cabin_wind = ss(20.0, 90.0, pl.airspeed);
    }
    if vehicle.is_some() != e.driving {
        e.driving = vehicle.is_some();
        e.grounded = true; // no landing thud from getting in or out
    }

    // ---- tree whooshes: each close tree once
    for t in &mut e.whooshed {
        t.1 -= dt;
    }
    e.whooshed.retain(|t| t.1 > 0.0);
    for (tp, scale) in close {
        let key = (tp * 0.5).round().as_ivec3();
        if e.whooshed.iter().any(|w| w.0 == key) {
            continue;
        }
        e.whooshed.push((key, 3.0));
        let side = (tp - pos).normalize_or_zero().dot(cam.right().as_vec3());
        audio.handle.event(Ev::Whoosh { speed, pan: side.clamp(-1.0, 1.0), size: (scale / 2.0).clamp(0.2, 1.0) });
    }

    // ---- thunderstorms: strikes at random distances (audio only)
    if matches!(code, 95..=99) && e.rand() < dt / 14.0 {
        let d = 300.0 + 9_000.0 * e.rand().powi(2);
        audio.handle.event(Ev::Thunder { distance_m: d });
    }

    e.prev_vel = vel;
    let mix = mix.map(|m| *m).unwrap_or_default();
    if mix.calm {
        focus(&mut p);
    }
    // 0.35 is the tuned scape level at the default master (see `settings::MASTER_REF`).
    p.master = 0.35 * mix.level();
    p.mix = mix.scape_mix();
    audio.handle.set(p);
}


/// Keep a place readable: the loudest couple of ambient beds play, the rest
/// sink into the background; only the two most present animal groups call.
fn focus(p: &mut Params) {
    const RANK: [f32; 4] = [1.0, 0.55, 0.3, 0.15];
    let mut beds: Vec<&mut f32> = vec![
        &mut p.traffic_far,
        &mut p.traffic_near,
        &mut p.traffic_minor,
        &mut p.rail,
        &mut p.surf,
        &mut p.lake,
        &mut p.river,
        &mut p.waterfall,
        &mut p.crowd,
        &mut p.industry,
    ];
    // Plants count as one bed ("the trees"), sharing the slot by their mix.
    let veg_sum: f32 = p.veg.iter().sum();
    let mut veg_slot = veg_sum.min(1.0);
    beds.push(&mut veg_slot);
    beds.sort_by(|a, b| b.total_cmp(a));
    for (i, b) in beds.into_iter().enumerate() {
        *b *= RANK[i.min(RANK.len() - 1)];
    }
    if veg_sum > 1e-4 {
        let k = veg_slot / veg_sum;
        p.veg.iter_mut().for_each(|v| *v *= k);
    }
    // Animals: the dominant group fully, the next at half, others silent.
    let mut order: Vec<usize> = (0..fauna::N).collect();
    order.sort_by(|&a, &b| p.fauna[b].total_cmp(&p.fauna[a]));
    for (r, &k) in order.iter().enumerate() {
        p.fauna[k] *= match r {
            0 => 0.8,
            1 => 0.4,
            _ => 0.0,
        };
    }
    p.hum *= 0.5;
}
