//! A short procedural sonic-boom burst, played when the player breaks Mach 1.
//!
//! The samples are synthesized once at startup (no asset file): a low N-wave
//! thump, a sharp crack and a noise tail. `player.rs` emits `SonicBoom`; this
//! plugin turns each message into a one-shot Bevy audio source.

use crate::player::SonicBoom;
use bevy::audio::{AddAudioSource, ChannelCount, Decodable, SampleRate, Source, Volume};
use bevy::prelude::*;
use std::num::NonZero;
use std::sync::Arc;
use std::time::Duration;

const RATE: u32 = 48_000;

#[derive(Asset, TypePath)]
pub struct BoomAudio {
    samples: Arc<[f32]>,
}

pub struct BoomSource {
    samples: Arc<[f32]>,
    pos: usize,
}

impl Iterator for BoomSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let s = self.samples.get(self.pos).copied();
        self.pos += 1;
        s
    }
}

impl Source for BoomSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        NonZero::new(1).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        NonZero::new(RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(self.samples.len() as f64 / RATE as f64))
    }
}

impl Decodable for BoomAudio {
    type Decoder = BoomSource;
    fn decoder(&self) -> BoomSource {
        BoomSource { samples: self.samples.clone(), pos: 0 }
    }
}

pub struct SonicBoomPlugin;

impl Plugin for SonicBoomPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<BoomAudio>()
            .add_systems(Startup, setup)
            .add_systems(Update, play_booms);
    }
}

#[derive(Resource, Clone)]
struct BoomSound(Handle<BoomAudio>);

fn setup(mut commands: Commands, mut assets: ResMut<Assets<BoomAudio>>) {
    commands.insert_resource(BoomSound(assets.add(BoomAudio { samples: synth_boom().into() })));
}

fn play_booms(mut commands: Commands, mut booms: MessageReader<SonicBoom>, sound: Res<BoomSound>, mix: Option<Res<crate::settings::AudioSettings>>) {
    // The boom is an air sound: master x the wind-and-rockets slider.
    let gain = mix.map(|m| m.level() * m.air.clamp(0.0, 1.0)).unwrap_or(1.0);
    for SonicBoom(pos) in booms.read() {
        commands.spawn((
            AudioPlayer(sound.0.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear((0.9 * gain).min(1.0))),
            Transform::from_translation(*pos),
            Name::new("sonic boom"),
        ));
    }
}

fn synth_boom() -> Vec<f32> {
    let n = (RATE as f32 * 1.35) as usize;
    let mut out = Vec::with_capacity(n);
    let mut seed = 0x1234_5678u32;
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        // Low pressure wave: pitch and level drop quickly.
        let thump_hz = 28.0 + 52.0 * (-t * 2.2).exp();
        let thump = (std::f32::consts::TAU * thump_hz * t).sin() * (-t * 3.0).exp();
        // The sharp leading crack.
        let crack = (std::f32::consts::TAU * 190.0 * t).sin() * (-t * 22.0).exp();
        // Broadband tail.
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        let mut s = thump * 0.75 + crack * 0.3 + noise * 0.3 * (-t * 5.0).exp();
        // A very short attack ramp avoids a speaker pop, then an N-wave doublet.
        let attack = (t / 0.003).min(1.0);
        let doublet = 1.0 - 0.7 * ((t - 0.045).max(0.0) / 0.02).min(1.0);
        s *= attack * doublet * (-t * 2.2).exp();
        out.push((s * 0.75).clamp(-1.0, 1.0));
    }
    out
}
