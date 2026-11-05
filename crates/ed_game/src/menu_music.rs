//! Main-menu music: the procedural lo-fi loop from `ed_audio::lofi`, audible
//! only on the main menu (`GameState::Menu`). It fades out when a game starts
//! and the world soundscape (see `soundscape.rs`) is paused while it plays.

use std::num::NonZero;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::audio::{AddAudioSource, ChannelCount, Decodable, SampleRate, Source};
use bevy::prelude::*;
use ed_audio::lofi::LoFi;

use crate::menu::GameState;
use crate::settings::AudioSettings;

/// Music level before the master volume.
const LEVEL: f32 = 0.5;

pub struct MenuMusicPlugin;

impl Plugin for MenuMusicPlugin {
    fn build(&self, app: &mut App) {
        let gain = Arc::new(AtomicU32::new(0));
        app.add_audio_source::<MusicAudio>()
            .insert_resource(MusicGain(gain.clone()))
            .insert_resource(PendingMusic(Mutex::new(Some(gain))))
            .add_systems(Startup, start_music)
            .add_systems(Update, drive_music);
    }
}

/// Target gain shared with the audio thread (f32 bits).
#[derive(Resource)]
struct MusicGain(Arc<AtomicU32>);

#[derive(Resource)]
struct PendingMusic(Mutex<Option<Arc<AtomicU32>>>);

#[derive(Asset, TypePath)]
struct MusicAudio {
    gain: Mutex<Option<Arc<AtomicU32>>>,
}

struct MusicSource {
    synth: LoFi,
    gain: Arc<AtomicU32>,
    frame: [f32; 2],
    ch: usize,
    tick: u32,
}

impl Iterator for MusicSource {
    type Item = f32;
    #[inline]
    fn next(&mut self) -> Option<f32> {
        if self.ch == 0 {
            // Re-read the shared target every ~10 ms, not every sample.
            if self.tick % 480 == 0 {
                self.synth.set_gain(f32::from_bits(self.gain.load(Ordering::Relaxed)));
            }
            self.tick = self.tick.wrapping_add(1);
            self.frame = self.synth.next_frame();
        }
        let s = self.frame[self.ch];
        self.ch ^= 1;
        Some(s)
    }
}

impl Source for MusicSource {
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

impl Decodable for MusicAudio {
    type Decoder = MusicSource;
    fn decoder(&self) -> MusicSource {
        let gain = self.gain.lock().ok().and_then(|mut g| g.take()).unwrap_or_else(|| Arc::new(AtomicU32::new(0)));
        MusicSource { synth: LoFi::new(), gain, frame: [0.0; 2], ch: 0, tick: 0 }
    }
}

fn start_music(mut commands: Commands, pending: Res<PendingMusic>, mut assets: ResMut<Assets<MusicAudio>>) {
    let Some(gain) = pending.0.lock().ok().and_then(|mut g| g.take()) else { return };
    let h = assets.add(MusicAudio { gain: Mutex::new(Some(gain)) });
    commands.spawn((AudioPlayer(h), PlaybackSettings::ONCE, Name::new("menu-music")));
}

fn drive_music(state: Res<State<GameState>>, audio: Option<Res<AudioSettings>>, gain: Res<MusicGain>) {
    let vol = audio.map(|a| if a.muted { 0.0 } else { a.master }).unwrap_or(0.7);
    let want = if *state.get() == GameState::Menu { LEVEL * vol } else { 0.0 };
    gain.0.store(want.to_bits(), Ordering::Relaxed);
}

/// Whether the main menu owns the speakers (the world soundscape stays quiet).
pub fn music_playing(state: &GameState) -> bool {
    *state == GameState::Menu
}
