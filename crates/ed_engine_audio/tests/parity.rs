//! The port against the JS (the source of truth): replay the controls the
//! kit's `render.mjs` fed its voice, and compare the Rust render with short
//! clips of the JS render (tests/data, made by tools/make_test_data.mjs).
//!
//! Windows: idle, the rev limiter, a full-throttle pull, and the lift-off
//! (blow-off valve, pops and bangs). Checked per window: loudness, spectral
//! centroid, octave-band energy shares, the strongest firing frequency
//! (pitch tracking) and waveform correlation. The Rust port is currently
//! sample-identical at 16-bit; the tolerances leave room for libm and
//! float-order differences on other platforms.

use ed_engine_audio::analysis::{band_shares, centroid, correlation, mono, peak_hz, rms};
use ed_engine_audio::controls::Controls;
use ed_engine_audio::{car, wav};
use std::path::PathBuf;

const WINDOWS: [(&str, f64, f64); 4] = [("idle", 2.0, 2.5), ("limiter", 5.5, 6.0), ("pull", 10.0, 10.5), ("lift", 17.15, 17.65)];

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(name)
}

fn check(id: &str) {
    let c = car(id).unwrap();
    let ctl = Controls::parse(&std::fs::read_to_string(data(&format!("{id}.controls.json"))).unwrap()).unwrap();
    let sr = ctl.sample_rate as f64;
    let (l, r) = ctl.render(c, Some(WINDOWS[3].2 + 0.01));
    // Quantize like the reference WAV (16-bit, then the mono fold).
    let q = |x: f32| ((x as f64 * 32767.0).round().clamp(-32767.0, 32767.0) / 32767.0) as f32;
    let l: Vec<f32> = l.into_iter().map(q).collect();
    let r: Vec<f32> = r.into_iter().map(q).collect();
    let rust = mono(&l, &r);
    let js = &wav::read(&data(&format!("{id}.ref.wav"))).unwrap().channels[0];
    let mut off = 0;
    for (name, a, b) in WINDOWS {
        let (i0, i1) = ((a * sr).round() as usize, (b * sr).round() as usize);
        let x = &rust[i0..i1];
        let y = &js[off..off + (i1 - i0)];
        off += i1 - i0;
        let (rx, ry) = (rms(x), rms(y));
        let db = 20.0 * (rx / ry).log10();
        assert!(db.abs() < 0.5, "{id} {name}: loudness {db:+.2} dB (rust {rx:.4}, js {ry:.4})");
        let (cx, cy) = (centroid(x, sr), centroid(y, sr));
        assert!((cx / cy - 1.0).abs() < 0.03, "{id} {name}: centroid rust {cx:.0} Hz vs js {cy:.0} Hz");
        let (bx, by) = (band_shares(x, sr), band_shares(y, sr));
        let bd: f64 = bx.iter().zip(&by).map(|(p, q)| (p - q).abs()).sum::<f64>() / 2.0;
        assert!(bd < 0.03, "{id} {name}: band energy shares differ by {bd:.3}\n rust {bx:.3?}\n js   {by:.3?}");
        let corr = correlation(x, y);
        assert!(corr > 0.98, "{id} {name}: waveform correlation {corr:.4}");
        if name == "pull" && c.kind == "piston" {
            // Pitch: the firing frequency the revs imply, found in both.
            let f = &ctl.frames[(a * 60.0) as usize + 15];
            let exp = f.rpm / 60.0 * c.firing.len() as f64 / 2.0;
            let (px, py) = (peak_hz(x, sr, exp * 0.75, exp * 1.25), peak_hz(y, sr, exp * 0.75, exp * 1.25));
            assert!((px - py).abs() <= 2.0, "{id}: firing pitch rust {px} Hz vs js {py} Hz (expected ~{exp:.0})");
        }
    }
}

#[test]
fn na_v8_mustang() {
    check("mustang");
}
#[test]
fn twin_turbo_v10_huracan() {
    check("huracan");
}
#[test]
fn race_flat_six_gt3() {
    check("gt3");
}
#[test]
fn race_f1() {
    check("f1");
}
#[test]
fn turbo_inline_four_civic() {
    check("civic");
}
#[test]
fn motorcycle_hayabusa() {
    check("hayabusa");
}
#[test]
fn jet() {
    check("jet");
}
