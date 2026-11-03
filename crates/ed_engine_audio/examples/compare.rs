//! Compare the Rust port with the JS reference renders:
//!
//!   node vendor/engine-sound-kit/render.mjs ref/ gt3 huracan jet
//!   cargo run --release -p ed_engine_audio --example compare -- ref/ out/ gt3 huracan jet
//!
//! For each car: replays `ref/<id>.controls.json` through the Rust voice,
//! writes `out/<id>.rust.wav`, and prints loudness, centroid, band shares and
//! pitch per section of the scripted drive, Rust next to JS.

use ed_engine_audio::analysis::{band_shares, centroid, correlation, mono, peak_hz, rms};
use ed_engine_audio::controls::Controls;
use ed_engine_audio::{car, wav};
use std::path::PathBuf;

/// Sections of render.mjs's script (seconds).
const SECTIONS: [(&str, f64, f64); 7] = [
    ("idle", 1.0, 3.0),
    ("rev", 3.0, 4.6),
    ("limiter", 4.6, 7.5),
    ("pull", 7.5, 17.0),
    ("lift", 17.0, 20.0),
    ("cruise", 20.0, 26.0),
    ("brake", 26.0, 32.0),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let refdir = PathBuf::from(&args[0]);
    let outdir = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&outdir).unwrap();
    for id in &args[2..] {
        let c = car(id).unwrap_or_else(|| panic!("no car {id}"));
        let ctl = Controls::parse(&std::fs::read_to_string(refdir.join(format!("{id}.controls.json"))).unwrap()).unwrap();
        let t0 = std::time::Instant::now();
        let (l, r) = ctl.render(c, None);
        let cpu = t0.elapsed().as_secs_f64() / (l.len() as f64 / ctl.sample_rate as f64);
        wav::write(&outdir.join(format!("{id}.rust.wav")), ctl.sample_rate, &l, Some(&r)).unwrap();
        let js = wav::read(&refdir.join(format!("{id}.wav"))).unwrap();
        let sr = ctl.sample_rate as f64;
        let rm = mono(&l, &r);
        let jm = mono(&js.channels[0], &js.channels[1]);
        println!("== {id}  cpu x realtime {cpu:.4}  whole: rms rust {:.4} js {:.4}  corr {:.4}", rms(&rm), rms(&jm), correlation(&rm, &jm));
        for (name, a, b) in SECTIONS {
            let (i0, i1) = ((a * sr) as usize, ((b * sr) as usize).min(rm.len()));
            let (xr, xj) = (&rm[i0..i1], &jm[i0..i1]);
            let (br, bj) = (band_shares(xr, sr), band_shares(xj, sr));
            let bd: f64 = br.iter().zip(&bj).map(|(x, y)| (x - y).abs()).sum::<f64>() / 2.0;
            println!(
                "  {name:8} rms {:.4} / {:.4} ({:+.2} dB)  centroid {:6.0} / {:6.0} Hz  band diff {:.3}  corr {:.3}",
                rms(xr), rms(xj), 20.0 * (rms(xr) / rms(xj)).log10(), centroid(xr, sr), centroid(xj, sr), bd, correlation(xr, xj)
            );
        }
        // Pitch tracking through the pull: strongest firing frequency per 0.5 s.
        let cyl = c.firing.len() as f64;
        let mut line = String::from("  pitch (Hz rust/js):");
        let mut t = 8.0;
        while t < 16.5 {
            let (i0, i1) = ((t * sr) as usize, ((t + 0.25) * sr) as usize);
            let f = &ctl.frames[(t * 60.0) as usize];
            let exp = f.rpm / 60.0 * cyl / 2.0 * if c.cycle == 360.0 { 2.0 } else { 1.0 };
            let (lo, hi) = ((exp * 0.7).max(10.0), exp * 1.3 + 5.0);
            line += &format!(" {:.0}/{:.0}", peak_hz(&rm[i0..i1], sr, lo, hi), peak_hz(&jm[i0..i1], sr, lo, hi));
            t += 1.0;
        }
        println!("{line}");
    }
}
