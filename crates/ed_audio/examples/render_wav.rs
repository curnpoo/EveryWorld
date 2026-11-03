//! Render engine demo clips: `cargo run -p ed_audio --example render_wav -- out_dir`
//! Each clip: 2 s idle, a 2.5 s rev to ~80% redline, 1.5 s back to idle.

use ed_audio::{EngineProfile, EngineSynth};
use std::io::Write;

fn write_wav(path: &std::path::Path, sr: u32, samples: &[f32]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    let data_len = (samples.len() * 2) as u32;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&sr.to_le_bytes())?;
    f.write_all(&(sr * 2).to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for s in samples {
        f.write_all(&((s.clamp(-1.0, 1.0) * 32_000.0) as i16).to_le_bytes())?;
    }
    Ok(())
}

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "engine_clips".into()));
    std::fs::create_dir_all(&out).unwrap();
    let sr = 44_100u32;
    for (file, p) in [
        ("inline4.wav", EngineProfile::inline4()),
        ("v8_crossplane.wav", EngineProfile::v8_crossplane()),
        ("flat6.wav", EngineProfile::flat6()),
        ("vtwin.wav", EngineProfile::vtwin()),
    ] {
        let (idle, red) = (p.idle_rpm, p.redline_rpm);
        let mut e = EngineSynth::new(p, sr as f32);
        let mut buf = Vec::new();
        let total = 6.0;
        let n = (total * sr as f32) as usize;
        for i in 0..n {
            let t = i as f32 / sr as f32;
            let (rpm, thr) = if t < 2.0 {
                (idle, 0.05)
            } else if t < 4.5 {
                let k = (t - 2.0) / 2.5;
                (idle + (red * 0.8 - idle) * k, 0.95)
            } else {
                let k = ((t - 4.5) / 1.5).min(1.0);
                (red * 0.8 + (idle - red * 0.8) * k, 0.0)
            };
            e.target_rpm = rpm;
            e.target_throttle = thr;
            // Fade in/out to avoid clicks.
            let fade = (t / 0.05).min(1.0) * ((total - t) / 0.1).min(1.0);
            buf.push(e.sample() * fade);
        }
        write_wav(&out.join(file), sr, &buf).unwrap();
        println!("wrote {}", out.join(file).display());
    }
}
