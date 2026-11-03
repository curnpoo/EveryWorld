//! Measurements used to compare the Rust port with the JS reference renders:
//! RMS, spectral centroid, energy per octave band, and the strongest
//! frequency in a range (pitch tracking). Plain code, no dependencies.

use std::f64::consts::PI;

pub fn rms(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|&v| v as f64 * v as f64).sum::<f64>() / x.len() as f64).sqrt()
}

/// In-place radix-2 FFT (re, im), length a power of two.
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f64;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0, 0.0);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let nr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = nr;
            }
        }
        len <<= 1;
    }
}

/// Averaged power spectrum (Hann windows of `n`, 50% overlap). Returns power
/// per bin 0..n/2, bin width sr / n.
pub fn power_spectrum(x: &[f32], n: usize) -> Vec<f64> {
    let mut acc = vec![0.0; n / 2 + 1];
    let win: Vec<f64> = (0..n).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos()).collect();
    let (mut re, mut im) = (vec![0.0; n], vec![0.0; n]);
    let mut count = 0;
    let mut start = 0;
    while start + n <= x.len() {
        for i in 0..n {
            re[i] = x[start + i] as f64 * win[i];
            im[i] = 0.0;
        }
        fft(&mut re, &mut im);
        for k in 0..=n / 2 {
            acc[k] += re[k] * re[k] + im[k] * im[k];
        }
        count += 1;
        start += n / 2;
    }
    if count > 0 {
        for v in &mut acc {
            *v /= count as f64;
        }
    }
    acc
}

/// Power-weighted mean frequency (Hz) between 20 Hz and 16 kHz.
pub fn centroid(x: &[f32], sr: f64) -> f64 {
    let n = 4096;
    let p = power_spectrum(x, n);
    let (mut num, mut den) = (0.0, 0.0);
    for (k, &v) in p.iter().enumerate() {
        let f = k as f64 * sr / n as f64;
        if (20.0..=16000.0).contains(&f) {
            num += f * v;
            den += v;
        }
    }
    if den > 0.0 { num / den } else { 0.0 }
}

/// Octave bands (lower edges in Hz) the comparisons use.
pub const BANDS: [f64; 10] = [20.0, 40.0, 80.0, 160.0, 320.0, 640.0, 1280.0, 2560.0, 5120.0, 10240.0];

/// Share of the power (20 Hz .. 20.48 kHz) in each octave band of [`BANDS`].
pub fn band_shares(x: &[f32], sr: f64) -> [f64; 10] {
    let n = 4096;
    let p = power_spectrum(x, n);
    let mut out = [0.0; 10];
    for (k, &v) in p.iter().enumerate() {
        let f = k as f64 * sr / n as f64;
        for b in 0..10 {
            if f >= BANDS[b] && f < BANDS[b] * 2.0 {
                out[b] += v;
            }
        }
    }
    let total: f64 = out.iter().sum();
    if total > 0.0 {
        for v in &mut out {
            *v /= total;
        }
    }
    out
}

/// Strongest frequency between lo and hi Hz (plain DFT, 1 Hz steps), like
/// `render.mjs`'s pitch check.
pub fn peak_hz(x: &[f32], sr: f64, lo: f64, hi: f64) -> f64 {
    let (mut best, mut best_hz) = (0.0, 0.0);
    let mut f = lo;
    while f <= hi {
        let w = 2.0 * PI * f / sr;
        let (mut re, mut im) = (0.0, 0.0);
        let (cw, sw) = (w.cos(), w.sin());
        let (mut c, mut s) = (1.0f64, 0.0f64);
        for &v in x {
            re += v as f64 * c;
            im += v as f64 * s;
            let nc = c * cw - s * sw;
            s = c * sw + s * cw;
            c = nc;
        }
        let m = re * re + im * im;
        if m > best {
            best = m;
            best_hz = f;
        }
        f += 1.0;
    }
    best_hz
}

/// Mono mix (L+R)/2.
pub fn mono(l: &[f32], r: &[f32]) -> Vec<f32> {
    l.iter().zip(r).map(|(a, b)| 0.5 * (a + b)).collect()
}

/// Pearson correlation of two equal-length signals.
pub fn correlation(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let ma = a[..n].iter().map(|&v| v as f64).sum::<f64>() / n as f64;
    let mb = b[..n].iter().map(|&v| v as f64).sum::<f64>() / n as f64;
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (x, y) = (a[i] as f64 - ma, b[i] as f64 - mb);
        sab += x * y;
        saa += x * x;
        sbb += y * y;
    }
    if saa > 0.0 && sbb > 0.0 { sab / (saa * sbb).sqrt() } else { 0.0 }
}
