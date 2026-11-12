//! The alpha's tone mapper on the CPU (P1-D18): a line-for-line transcription of common.slang
//! `alpha_tonemap` (itself the alpha's `resolve_fs`, reference/alpha_metal world.metal), used
//! by the GPU-vs-CPU tolerance test and anything that needs display values off the GPU.
//!
//! The film curve is the alpha's 64^3 RGBA16F LUT (`assets/filmic_lut_rgba16f.bin`, x fastest):
//! Blender's Filmic base contrast in its "-11..12" log2 encoding around 0.18, as shipped by
//! Bevy (`bevy_core_pipeline` tonemapping LUTs, MIT/Apache-2.0; Filmic by Troy Sobotka).

/// Texels per LUT side.
pub const LUT_N: usize = 64;
/// The LUT, RGBA16F, x fastest (`LUT_N`^3 texels, 8 bytes each).
pub static FILMIC_LUT: &[u8] = include_bytes!("../assets/filmic_lut_rgba16f.bin");

/// IEEE half -> f32.
pub fn f16_to_f32(h: u16) -> f32 {
    let s = ((h >> 15) as u32) << 31;
    let e = ((h >> 10) & 0x1F) as u32;
    let m = (h & 0x3FF) as u32;
    let bits = match (e, m) {
        (0, 0) => s,
        (0, _) => {
            // Subnormal: normalise.
            let mut e = 127 - 15 + 1;
            let mut m = m;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            s | (e << 23) | ((m & 0x3FF) << 13)
        }
        (31, 0) => s | 0x7F80_0000,
        (31, _) => s | 0x7FC0_0000,
        _ => s | ((e + 127 - 15) << 23) | (m << 13),
    };
    f32::from_bits(bits)
}

fn texel(t: [usize; 3]) -> [f32; 3] {
    let i = ((t[2] * LUT_N + t[1]) * LUT_N + t[0]) * 8;
    let h = |o: usize| f16_to_f32(u16::from_le_bytes([FILMIC_LUT[i + o], FILMIC_LUT[i + o + 1]]));
    [h(0), h(2), h(4)]
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// The film curve (common.slang `filmic`).
pub fn filmic(x: [f32; 3]) -> [f32; 3] {
    let mut n = [0.0f32; 3];
    for k in 0..3 {
        let mut v = x[k].max(0.0);
        if v < 0.000_030_517_57 {
            v += 0.000_015_258_78;
        }
        n[k] = ((((v / 0.18).log2() + 11.0) / 23.0).clamp(0.0, 1.0)) * (LUT_N - 1) as f32;
    }
    let i = [0, 1, 2].map(|k| (n[k] as usize).min(LUT_N - 2));
    let g = [0, 1, 2].map(|k| n[k] - i[k] as f32);
    let at = |dx, dy, dz| texel([i[0] + dx, i[1] + dy, i[2] + dz]);
    let c00 = lerp3(at(0, 0, 0), at(1, 0, 0), g[0]);
    let c10 = lerp3(at(0, 1, 0), at(1, 1, 0), g[0]);
    let c01 = lerp3(at(0, 0, 1), at(1, 0, 1), g[0]);
    let c11 = lerp3(at(0, 1, 1), at(1, 1, 1), g[0]);
    lerp3(lerp3(c00, c10, g[1]), lerp3(c01, c11, g[1]), g[2])
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Scene radiance -> display value (gamma 2.2 encoded, 0..1), as the alpha wrote it.
pub fn alpha_tonemap(hdr: [f32; 3], exposure: f32) -> [f32; 3] {
    let wb = [1.10, 1.0, 0.86];
    let c = filmic([0, 1, 2].map(|k| hdr[k] * exposure * wb[k]));
    let l = c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722;
    let s = smoothstep(0.35, 0.9, l);
    let tint = [1.03, 1.0, 0.95];
    [0, 1, 2].map(|k| (c[k] * (1.0 + (tint[k] - 1.0) * s)).clamp(0.0, 1.0).powf(1.0 / 2.2))
}

/// sRGB transfer (decode), as tonemap.slang `srgb_decode`.
pub fn srgb_decode(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Band colours of [`test_ramp`] (tonemap.slang `test_ramp`): grey, R, G, B, 12 saturated hues.
pub const RAMP_BANDS: [[f32; 3]; 16] = [
    [1.0, 1.0, 1.0],
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [1.0, 1.0, 0.0],
    [0.0, 1.0, 1.0],
    [1.0, 0.0, 1.0],
    [1.0, 0.5, 0.0],
    [0.5, 1.0, 0.0],
    [0.0, 1.0, 0.5],
    [0.0, 0.5, 1.0],
    [0.5, 0.0, 1.0],
    [1.0, 0.0, 0.5],
    [1.0, 0.2, 0.1],
    [0.1, 0.6, 0.2],
    [0.2, 0.3, 1.0],
];

/// The HDR value the `SceneMode::ToneRamp` frame tone maps at pixel (x, y) of a w x h view:
/// 16 row bands, each a 16-stop ramp 2^-12 .. 2^4 left to right.
pub fn test_ramp(x: u32, y: u32, w: u32, h: u32) -> [f32; 3] {
    let band = ((y * 16) / h).min(15) as usize;
    let t = x as f32 / (w.max(2) - 1) as f32;
    let k = (-12.0 + 16.0 * t).exp2();
    RAMP_BANDS[band].map(|c| c * k)
}

/// The alpha's exposure.
pub const ALPHA_EXPOSURE: f32 = 1.7;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_decoding() {
        assert_eq!(f16_to_f32(0x3C00), 1.0);
        assert_eq!(f16_to_f32(0xC000), -2.0);
        assert_eq!(f16_to_f32(0x0001), 2.0f32.powi(-24));
        assert_eq!(f16_to_f32(0x7BFF), 65504.0);
    }

    #[test]
    fn curve_is_monotonic_black_to_white() {
        assert_eq!(FILMIC_LUT.len(), LUT_N * LUT_N * LUT_N * 8);
        let mut prev = -1.0;
        for i in 0..200 {
            let x = 2f32.powf(-12.0 + i as f32 * 0.1);
            let v = alpha_tonemap([x, x, x], 1.0)[1];
            assert!(v >= prev - 1e-6, "not monotonic at {x}");
            prev = v;
        }
        assert!(alpha_tonemap([0.0; 3], 1.0)[1] < 0.05);
        assert!(alpha_tonemap([1000.0; 3], 1.0)[1] > 0.95);
    }
}
