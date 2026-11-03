//! Procedural engine audio for EveryWorld: a Rust port of the owner's engine
//! sound kit (`vendor/engine-sound-kit`, the JS is the source of truth).
//!
//! - [`EngineVoice`] (`synth.js`) and the special generators (`special.js`:
//!   jet, pod racer, warp drive, steam train) render one vehicle.
//! - [`cars`] holds every vehicle of the kit, generated from `cars.js`.
//! - [`driver`] turns a game's own gearbox state (rpm, gear, throttle, speed)
//!   into the voice's controls and events.
//! - [`mixer`] hosts several voices on the audio thread with distance gain and
//!   panning, fed lock-free from the game thread.
//! - [`analysis`] measures renders (RMS, spectral centroid, band energies) for
//!   the parity tests against the JS reference.
//!
//! Engine-free: no Bevy here. `ed_game` wraps [`mixer::Mixer`] in a Bevy audio
//! source.

pub mod analysis;
pub mod cars;
pub mod controls;
pub mod driver;
pub mod dsp;
pub mod mixer;
pub mod special;
pub mod voice;
pub mod wav;

pub use cars::{car, car_index, catalog, Car};
pub use voice::{EngineVoice, Event, Params};

/// The rate the game renders at (rodio resamples to the device if needed).
pub const SAMPLE_RATE: u32 = 48_000;
