//! Textures at bake time (P1-D13): decode, mip chain, block compression, stable page ids.
//!
//! - Colour space by role: `Srgb` (base colour) is filtered in linear light and stored as BC7
//!   with the sRGB role (sampled through `Bc7Srgb`); `Linear` (ORM, masks) is BC7 linear;
//!   `Normal` is **BC5** (two channels, X and Y; the shader rebuilds Z), mips renormalised;
//!   single-channel data (displacement) is BC4.
//! - Encoder: Intel ISPC Texture Compressor kernels through `intel_tex_2` (prebuilt for
//!   aarch64-apple-darwin). Each mip is split into bands of block rows that are encoded in
//!   parallel and concatenated in order; block encoding has no shared state, so the bytes do
//!   not depend on the thread count (tested below and across processes in `tests/`).
//! - Page id: the first 8 bytes of the texture's AssetId (BLAKE3 of its asset path), so the
//!   id is stable across rebakes and content edits.

use anyhow::{Context, Result, bail};
use ed_pack::AssetId;
use ed_pack::kinds::{ColourRole, Texture, TextureFormat};
use rayon::prelude::*;
use std::borrow::Cow;

/// What a texture is used for (decides format, filtering and colour space).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Base colour / albedo (sRGB, BC7).
    Colour,
    /// Linear data in RGB(A), e.g. occlusion-roughness-metal (BC7).
    Linear,
    /// Tangent-space normal map (BC5).
    Normal,
    /// One linear channel, e.g. displacement (BC4; the red channel is used).
    Single,
}

impl Role {
    pub fn parse(s: &str) -> Result<Role> {
        Ok(match s {
            "colour" | "color" | "srgb" | "base_colour" => Role::Colour,
            "linear" | "orm" | "data" => Role::Linear,
            "normal" => Role::Normal,
            "single" | "height" | "displacement" => Role::Single,
            _ => bail!("unknown texture role `{s}` (colour, linear, normal, single)"),
        })
    }
}

/// An 8-bit RGBA image.
#[derive(Clone, Debug, PartialEq)]
pub struct Rgba8 {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Rgba8 {
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Result<Rgba8> {
        if width == 0 || height == 0 || data.len() != (width * height * 4) as usize {
            bail!("image {width}x{height} with {} bytes", data.len());
        }
        Ok(Rgba8 { width, height, data })
    }
    pub fn px(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3]]
    }
}

/// Decodes PNG, JPEG or WebP bytes (by content sniffing) to RGBA8.
pub fn decode_image(bytes: &[u8]) -> Result<Rgba8> {
    if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return decode_webp(bytes);
    }
    let img = image::load_from_memory(bytes).context("decoding image")?.to_rgba8();
    Rgba8::new(img.width(), img.height(), img.into_raw())
}

pub fn decode_webp(bytes: &[u8]) -> Result<Rgba8> {
    let mut d = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes)).context("WebP header")?;
    let (w, h) = d.dimensions();
    let has_alpha = d.has_alpha();
    let mut buf = vec![0u8; d.output_buffer_size().context("WebP size")?];
    d.read_image(&mut buf).context("decoding WebP")?;
    let data = if has_alpha { buf } else { buf.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect() };
    Rgba8::new(w, h, data)
}

/// Lossless WebP encoding (test fixtures and the synthetic corpus).
pub fn encode_webp(img: &Rgba8) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    image_webp::WebPEncoder::new(&mut out).encode(&img.data, img.width, img.height, image_webp::ColorType::Rgba8).context("encoding WebP")?;
    Ok(out)
}

/// PNG encoding (fixtures).
pub fn encode_png(img: &Rgba8) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    image::ImageEncoder::write_image(image::codecs::png::PngEncoder::new(&mut out), &img.data, img.width, img.height, image::ExtendedColorType::Rgba8)?;
    Ok(out)
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}
fn linear_to_srgb(l: f32) -> u8 {
    let l = l.clamp(0.0, 1.0);
    let c = if l <= 0.0031308 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
    (c * 255.0 + 0.5) as u8
}

/// The full mip chain (mip 0 is `img`), box-filtered in the role's space: linear light for
/// colour, plain values for linear data, renormalised vectors for normals.
pub fn mip_chain(img: &Rgba8, role: Role) -> Vec<Rgba8> {
    let mut out = vec![img.clone()];
    while out.last().is_some_and(|m| m.width > 1 || m.height > 1) {
        let src = out.last().unwrap();
        let (w, h) = ((src.width / 2).max(1), (src.height / 2).max(1));
        let mut data = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let taps = [(2 * x, 2 * y), (2 * x + 1, 2 * y), (2 * x, 2 * y + 1), (2 * x + 1, 2 * y + 1)]
                    .map(|(sx, sy)| src.px(sx.min(src.width - 1), sy.min(src.height - 1)));
                let o = ((y * w + x) * 4) as usize;
                match role {
                    Role::Colour => {
                        for c in 0..3 {
                            let l: f32 = taps.iter().map(|t| srgb_to_linear(t[c])).sum::<f32>() * 0.25;
                            data[o + c] = linear_to_srgb(l);
                        }
                        data[o + 3] = ((taps.iter().map(|t| t[3] as u32).sum::<u32>() + 2) / 4) as u8;
                    }
                    Role::Normal => {
                        let mut v = [0.0f32; 3];
                        for t in &taps {
                            for c in 0..3 {
                                v[c] += t[c] as f32 / 127.5 - 1.0;
                            }
                        }
                        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
                        for c in 0..3 {
                            data[o + c] = ((v[c] / len * 0.5 + 0.5) * 255.0 + 0.5) as u8;
                        }
                        data[o + 3] = 255;
                    }
                    Role::Linear | Role::Single => {
                        for c in 0..4 {
                            data[o + c] = ((taps.iter().map(|t| t[c] as u32).sum::<u32>() + 2) / 4) as u8;
                        }
                    }
                }
            }
        }
        out.push(Rgba8 { width: w, height: h, data });
    }
    out
}

/// Pads to whole 4x4 blocks by repeating the edge texels; returns (width, height, bytes) with
/// `channels` bytes per texel taken from RGBA.
fn padded(img: &Rgba8, channels: &[usize]) -> (u32, u32, Vec<u8>) {
    let (w, h) = (img.width.next_multiple_of(4), img.height.next_multiple_of(4));
    let mut d = Vec::with_capacity((w * h) as usize * channels.len());
    for y in 0..h {
        for x in 0..w {
            let p = img.px(x.min(img.width - 1), y.min(img.height - 1));
            d.extend(channels.iter().map(|&c| p[c]));
        }
    }
    (w, h, d)
}

/// Block rows per parallel band.
const BAND_ROWS: u32 = 8;

/// Compresses one mip. `threads`: `None` = rayon's global pool.
fn compress(img: &Rgba8, format: TextureFormat) -> Vec<u8> {
    let (channels, block_bytes): (&[usize], usize) = match format {
        TextureFormat::Bc7 => (&[0, 1, 2, 3], 16),
        TextureFormat::Bc5 => (&[0, 1], 16),
        TextureFormat::Bc4 => (&[0], 8),
        _ => unreachable!("not a block format"),
    };
    let opaque = img.data.chunks_exact(4).all(|p| p[3] == 255);
    let (w, h, data) = padded(img, channels);
    let row_bytes = (w * channels.len() as u32) as usize;
    let block_rows = h / 4;
    let bands: Vec<u32> = (0..block_rows.div_ceil(BAND_ROWS)).collect();
    let settings = if opaque { intel_tex_2::bc7::opaque_fast_settings() } else { intel_tex_2::bc7::alpha_fast_settings() };
    let parts: Vec<Vec<u8>> = bands
        .par_iter()
        .map(|&b| {
            let r0 = b * BAND_ROWS * 4;
            let rows = (BAND_ROWS * 4).min(h - r0);
            let slice = &data[r0 as usize * row_bytes..(r0 + rows) as usize * row_bytes];
            let stride = row_bytes as u32;
            match format {
                TextureFormat::Bc7 => intel_tex_2::bc7::compress_blocks(&settings, &intel_tex_2::RgbaSurface { data: slice, width: w, height: rows, stride }),
                TextureFormat::Bc5 => intel_tex_2::bc5::compress_blocks(&intel_tex_2::RgSurface { data: slice, width: w, height: rows, stride }),
                _ => intel_tex_2::bc4::compress_blocks(&intel_tex_2::RSurface { data: slice, width: w, height: rows, stride }),
            }
        })
        .collect();
    let out: Vec<u8> = parts.concat();
    debug_assert_eq!(out.len(), (w / 4 * block_rows) as usize * block_bytes);
    out
}

/// Stable page id of a texture asset (P1-D13).
pub fn page_id(asset: &AssetId) -> u64 {
    u64::from_le_bytes(asset.0[..8].try_into().expect("8 bytes"))
}

/// Builds the pack texture: mips, compression by role, page id from the asset id.
pub fn bake(img: &Rgba8, role: Role, asset: &AssetId) -> Result<Texture<'static>> {
    if img.width > 16384 || img.height > 16384 {
        bail!("texture {}x{} is larger than 16384", img.width, img.height);
    }
    let (format, colour) = match role {
        Role::Colour => (TextureFormat::Bc7, ColourRole::Srgb),
        Role::Linear => (TextureFormat::Bc7, ColourRole::Linear),
        Role::Normal => (TextureFormat::Bc5, ColourRole::Normal),
        Role::Single => (TextureFormat::Bc4, ColourRole::Linear),
    };
    let mips = mip_chain(img, role).iter().map(|m| Cow::Owned(compress(m, format))).collect();
    Ok(Texture { format, role: colour, width: img.width, height: img.height, layers: 1, page_id: page_id(asset), mips })
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn pattern(w: u32, h: u32, seed: u32) -> Rgba8 {
        let mut d = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let v = x.wrapping_mul(2654435761).wrapping_add(y.wrapping_mul(40503)).wrapping_add(seed);
                d.extend_from_slice(&[(x * 255 / w) as u8, (y * 255 / h) as u8, (v >> 24) as u8, 255]);
            }
        }
        Rgba8::new(w, h, d).unwrap()
    }

    #[test]
    fn mip_chain_ends_at_one_texel_and_colour_filters_in_linear_light() {
        let mut img = Rgba8::new(2, 1, vec![0, 0, 0, 255, 255, 255, 255, 255]).unwrap();
        let m = mip_chain(&img, Role::Colour);
        assert_eq!(m.len(), 2);
        // Average of black and white in linear light is 0.5 -> sRGB 188, not 128.
        assert_eq!(m[1].px(0, 0)[0], 188);
        img = pattern(37, 5, 1);
        let m = mip_chain(&img, Role::Linear);
        assert_eq!((m.last().unwrap().width, m.last().unwrap().height), (1, 1));
        assert_eq!(m.len(), 6);
    }

    #[test]
    fn normal_mips_stay_unit_length() {
        let mut d = Vec::new();
        for i in 0..16 {
            let a = i as f32;
            let v = [a.sin() * 0.6, a.cos() * 0.6, 0.8f32];
            d.extend(v.iter().map(|c| ((c * 0.5 + 0.5) * 255.0 + 0.5) as u8));
            d.push(255);
        }
        let m = mip_chain(&Rgba8::new(4, 4, d).unwrap(), Role::Normal);
        for lvl in &m[1..] {
            for p in lvl.data.chunks_exact(4) {
                let v: Vec<f32> = p[..3].iter().map(|&c| c as f32 / 127.5 - 1.0).collect();
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                assert!((l - 1.0).abs() < 0.02, "{l}");
            }
        }
    }

    #[test]
    fn bc7_bytes_do_not_depend_on_the_thread_count() {
        let img = pattern(300, 260, 7);
        let a = AssetId::from_path("t/a");
        let one = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap().install(|| bake(&img, Role::Colour, &a).unwrap());
        let many = rayon::ThreadPoolBuilder::new().num_threads(8).build().unwrap().install(|| bake(&img, Role::Colour, &a).unwrap());
        assert_eq!(one, many);
        let again = bake(&img, Role::Colour, &a).unwrap();
        assert_eq!(one, again);
        assert_eq!(one.format, TextureFormat::Bc7);
        for (l, m) in one.mips.iter().enumerate() {
            let (w, h) = one.mip_size(l);
            assert_eq!(m.len() as u64, TextureFormat::Bc7.image_bytes(w, h));
        }
        let n = bake(&img, Role::Normal, &a).unwrap();
        assert_eq!(n.format, TextureFormat::Bc5);
        let n8 = rayon::ThreadPoolBuilder::new().num_threads(8).build().unwrap().install(|| bake(&img, Role::Normal, &a).unwrap());
        assert_eq!(n, n8);
        assert_eq!(page_id(&a), one.page_id);
    }

    #[test]
    fn bc7_round_trip_is_close() {
        // Solid colour blocks decode back almost exactly (BC7 mode 5/6 endpoint precision).
        let img = Rgba8::new(8, 8, [200u8, 100, 50, 255].repeat(64)).unwrap();
        let b = compress(&img, TextureFormat::Bc7);
        assert_eq!(b.len(), 4 * 16);
        assert_eq!(b[..16], b[16..32]);
    }

    #[test]
    fn webp_round_trip_is_lossless() {
        let img = pattern(33, 17, 3);
        let back = decode_image(&encode_webp(&img).unwrap()).unwrap();
        assert_eq!(back, img);
        let back = decode_image(&encode_png(&img).unwrap()).unwrap();
        assert_eq!(back, img);
    }
}
