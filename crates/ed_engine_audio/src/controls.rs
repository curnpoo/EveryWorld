//! Replays a `<id>.controls.json` written by the kit's `render.mjs`: the
//! exact control values (every 1/60 s) and events the JS reference render
//! fed its voice. Feeding them to the Rust voice is how the port is checked.

use crate::cars::Car;
use crate::voice::{EngineVoice, Event, Params};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Controls {
    pub sample_rate: u32,
    pub control_rate: u32,
    pub seed: u32,
    pub character: f64,
    pub crackle: f64,
    pub frames: Vec<Frame>,
}

/// One control frame. Keys left out (the trimmed test fixtures drop zeros
/// and empty lists) read as 0 / empty.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Frame {
    pub t: f64,
    pub rpm: f64,
    pub load: f64,
    pub overrun: f64,
    pub cut: f64,
    pub starter: f64,
    pub shaft: f64,
    pub horn: f64,
    pub pull_away: f64,
    pub clicks: Vec<f64>,
    pub events: Vec<(String, f64)>,
}

impl Controls {
    pub fn parse(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    /// Render the whole run the way `render.mjs` does: per frame, clicks,
    /// then events, then the params, then `sampleRate / 60` frames of audio.
    /// `seconds` limits the length (None: as many frames as there are).
    pub fn render(&self, car: &'static Car, seconds: Option<f64>) -> (Vec<f32>, Vec<f32>) {
        let sr = self.sample_rate as usize;
        let chunk = (sr as f64 / self.control_rate as f64).round() as usize;
        let total = self.frames.len() * chunk;
        let n = seconds.map_or(total, |s| ((s * sr as f64) as usize).min(total));
        let mut voice = EngineVoice::new(sr as f64, car, self.seed);
        voice.set_user(self.character, self.crackle);
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        let mut i = 0;
        for f in &self.frames {
            if i >= n {
                break;
            }
            for &c in &f.clicks {
                voice.click(c);
            }
            for (ty, amt) in &f.events {
                if let Some(ev) = Event::parse(ty) {
                    voice.event(ev, *amt);
                }
            }
            voice.set_params(&Params {
                rpm: f.rpm,
                load: f.load,
                overrun: f.overrun,
                cut: f.cut,
                starter: f.starter,
                shaft: f.shaft,
                horn: f.horn,
                pull_away: f.pull_away,
                body: 0.0,
            });
            let m = chunk.min(n - i);
            voice.render(&mut l[i..i + m], &mut r[i..i + m]);
            i += m;
        }
        (l, r)
    }
}
