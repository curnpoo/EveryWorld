//! Procedural night sky: an equirectangular (RA x Dec) RGBA8 texture with
//! faint stars, a few bright ones, and a Milky Way band along the galactic
//! plane. Engine-free and deterministic.
//!
//! Texture u = right ascension / 360°, v = (90° - declination) / 180°.

use crate::{hash01, noise};
use glam::Vec3;
use std::f32::consts::{PI, TAU};

/// North galactic pole and galactic centre (J2000 equatorial).
const POLE_RA: f32 = 192.859_5;
const POLE_DEC: f32 = 27.128_3;
const CENTRE_RA: f32 = 266.405;
const CENTRE_DEC: f32 = -28.936;

/// Unit vector for equatorial coordinates in degrees (x to RA 0, z to the pole).
pub fn eci_dir(ra_deg: f32, dec_deg: f32) -> Vec3 {
    let (ra, dec) = (ra_deg.to_radians(), dec_deg.to_radians());
    Vec3::new(dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin())
}

/// Milky Way brightness in 0..1 for a direction (galactic plane band,
/// brightest toward Sagittarius, broken up by dust lanes).
pub fn milky_way(d: Vec3) -> f32 {
    let pole = eci_dir(POLE_RA, POLE_DEC);
    let centre = eci_dir(CENTRE_RA, CENTRE_DEC);
    let b = d.dot(pole).clamp(-1.0, 1.0).asin(); // galactic latitude
    let band = (-(b / 0.17).powi(2)).exp();
    let core = d.dot(centre).max(0.0).powi(3);
    let n = noise::fbm3(d.x * 5.0, d.y * 5.0, d.z * 5.0, 11, 5);
    let fine = noise::fbm3(d.x * 18.0, d.y * 18.0, d.z * 18.0, 12, 4);
    // Dust lanes eat into the band, mostly near the plane.
    let dust = noise::smoothstep(0.52, 0.72, noise::fbm3(d.x * 9.0, d.y * 9.0, d.z * 9.0, 13, 4));
    let v = band * (0.35 + 0.65 * n) * (0.55 + 0.9 * core) * (1.0 - 0.7 * dust * band) * (0.75 + 0.5 * fine);
    v.clamp(0.0, 1.0)
}

/// Black-body-ish star tint for a 0..1 "temperature" (red .. white .. blue).
fn star_tint(t: f32) -> Vec3 {
    let red = Vec3::new(1.0, 0.62, 0.45);
    let white = Vec3::new(1.0, 0.97, 0.92);
    let blue = Vec3::new(0.66, 0.78, 1.0);
    if t < 0.5 { red.lerp(white, t * 2.0) } else { white.lerp(blue, (t - 0.5) * 2.0) }
}

/// Generate the sky texture, `w x h` RGBA8 (sRGB-encoded). `w` should be `2 h`.
pub fn generate(w: usize, h: usize) -> Vec<u8> {
    let mut lin = vec![Vec3::ZERO; w * h];
    // Milky Way: sampled on a coarse grid (it is smooth), then bilinearly spread.
    let (cw, ch) = (w / 4, h / 4);
    let mut mw = vec![0.0f32; cw * ch];
    for y in 0..ch {
        for x in 0..cw {
            let ra = (x as f32 + 0.5) / cw as f32 * 360.0;
            let dec = 90.0 - (y as f32 + 0.5) / ch as f32 * 180.0;
            mw[y * cw + x] = milky_way(eci_dir(ra, dec));
        }
    }
    for y in 0..h {
        let fy = ((y as f32 + 0.5) / h as f32 * ch as f32 - 0.5).clamp(0.0, (ch - 1) as f32);
        let (y0, ty) = (fy as usize, fy.fract());
        let y1 = (y0 + 1).min(ch - 1);
        for x in 0..w {
            let fx = (x as f32 + 0.5) / w as f32 * cw as f32 - 0.5;
            let x0 = fx.floor();
            let tx = fx - x0;
            let xa = (x0 as i32).rem_euclid(cw as i32) as usize;
            let xb = (xa + 1) % cw;
            let a = mw[y0 * cw + xa] * (1.0 - tx) + mw[y0 * cw + xb] * tx;
            let b = mw[y1 * cw + xa] * (1.0 - tx) + mw[y1 * cw + xb] * tx;
            let m = a * (1.0 - ty) + b * ty;
            // Warm core, cool arms.
            lin[y * w + x] = Vec3::new(0.95, 0.86, 0.78).lerp(Vec3::new(0.62, 0.72, 0.95), (1.0 - m).powi(2) * 0.8) * (m * m * 0.30 + m * 0.05);
        }
    }
    // Stars: uniform on the sphere, density boosted toward the galactic plane.
    let pole = eci_dir(POLE_RA, POLE_DEC);
    let n_stars = (w * h) / 120;
    for i in 0..n_stars as u64 {
        let ra = hash01(i, 1) * 360.0;
        let dec = (1.0 - 2.0 * hash01(i, 2)).asin().to_degrees(); // uniform on sphere
        let d = eci_dir(ra, dec);
        let lat = d.dot(pole).abs();
        if hash01(i, 7) > 0.35 + 0.65 * (1.0 - lat) {
            continue;
        }
        // Magnitude-like power law: lots of faint, very few bright.
        let r = hash01(i, 3);
        let bright = 0.04 + 1.4 * r.powi(9);
        let tint = star_tint(hash01(i, 4));
        let radius = 0.7 + 1.4 * r.powi(9);
        let cx = ra / 360.0 * w as f32;
        let cy = (90.0 - dec) / 180.0 * h as f32;
        // Longitude pixels shrink toward the poles; stretch the splat to stay round.
        let sx = radius / dec.to_radians().cos().max(0.15);
        let (x0, x1) = ((cx - sx * 2.5).floor() as i32, (cx + sx * 2.5).ceil() as i32);
        let (y0, y1) = ((cy - radius * 2.5).floor() as i32, (cy + radius * 2.5).ceil() as i32);
        for yy in y0.max(0)..=y1.min(h as i32 - 1) {
            for xx in x0..=x1 {
                let (dx, dy) = ((xx as f32 + 0.5 - cx) / sx, (yy as f32 + 0.5 - cy) / radius);
                let f = (-(dx * dx + dy * dy) * 1.6).exp();
                if f < 0.01 {
                    continue;
                }
                let xw = xx.rem_euclid(w as i32) as usize;
                lin[yy as usize * w + xw] += tint * (bright * f);
            }
        }
    }
    let enc = |c: f32| {
        let c = c.clamp(0.0, 1.0);
        let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
        (s * 255.0 + 0.5) as u8
    };
    let mut out = Vec::with_capacity(w * h * 4);
    for c in lin {
        out.extend_from_slice(&[enc(c.x), enc(c.y), enc(c.z), 255]);
    }
    out
}

/// Equirect UVs for a unit direction in equatorial coordinates.
pub fn uv_for(d: Vec3) -> (f32, f32) {
    let ra = d.y.atan2(d.x).rem_euclid(TAU);
    (ra / TAU, 0.5 - d.z.clamp(-1.0, 1.0).asin() / PI)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_is_brighter_than_poles() {
        let on_plane = eci_dir(CENTRE_RA, CENTRE_DEC);
        let pole = eci_dir(POLE_RA, POLE_DEC);
        assert!(milky_way(on_plane) > milky_way(pole) + 0.05, "{} vs {}", milky_way(on_plane), milky_way(pole));
    }

    #[test]
    fn texture_has_stars_and_dark_sky() {
        let (w, h) = (256, 128);
        let t = generate(w, h);
        assert_eq!(t.len(), w * h * 4);
        let lum: Vec<u8> = t.chunks(4).map(|p| p[0].max(p[1]).max(p[2])).collect();
        assert!(lum.iter().any(|&l| l > 120), "no bright stars");
        let dark = lum.iter().filter(|&&l| l < 30).count();
        assert!(dark > lum.len() / 2, "sky should be mostly dark");
    }

    #[test]
    fn uv_roundtrip() {
        let d = eci_dir(123.0, -40.0);
        let (u, v) = uv_for(d);
        assert!((u * 360.0 - 123.0).abs() < 1e-2 && ((0.5 - v) * 180.0 + 40.0).abs() < 1e-2);
    }
}
