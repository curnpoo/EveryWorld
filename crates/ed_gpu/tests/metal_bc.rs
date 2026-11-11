//! Block-compressed textures (M3): BC7 (colour, ORM) and BC5 (normals) upload with mips and
//! sample correctly. Gated on `Caps::bc_textures`. Compiles to nothing off macOS.
#![cfg(target_os = "macos")]

mod common;
use common::*;
use ed_gpu::ResourceState::*;
use ed_gpu::*;

/// LSB-first bit writer for hand-built blocks.
struct Bits {
    out: [u8; 16],
    at: usize,
}
impl Bits {
    fn put(&mut self, v: u32, n: usize) {
        for i in 0..n {
            if v >> i & 1 == 1 {
                self.out[(self.at + i) / 8] |= 1 << ((self.at + i) % 8);
            }
        }
        self.at += n;
    }
}

/// A solid BC7 mode-6 block: 7-bit endpoints (both equal) plus a shared p-bit, indices 0.
/// Each channel decodes to `c7 << 1 | p`.
fn bc7_solid(rgba7: [u32; 4], p: u32) -> [u8; 16] {
    let mut b = Bits { out: [0; 16], at: 0 };
    b.put(1 << 6, 7); // mode 6
    for c in rgba7 {
        b.put(c, 7);
        b.put(c, 7);
    }
    b.put(p, 1);
    b.put(p, 1);
    b.out
}

/// A solid BC5 block: two BC4 halves with equal endpoints and zero indices.
fn bc5_solid(r: u8, g: u8) -> [u8; 16] {
    let mut o = [0u8; 16];
    o[0] = r;
    o[1] = r;
    o[8] = g;
    o[9] = g;
    o
}

fn bc_texture(dev: &dyn Device, format: Format, size: u32, mips: u32, block: impl Fn(u32) -> [u8; 16]) -> TextureId {
    let t = dev
        .create_texture(&TextureDesc {
            label: "bc",
            kind: TextureKind::D2,
            format,
            width: size,
            height: size,
            depth_or_layers: 1,
            mips,
            samples: 1,
            usage: Usage { shader_read: true, copy_dst: true, ..usage() },
        })
        .unwrap();
    for m in 0..mips {
        let blocks = mip_size(size, m).div_ceil(4).pow(2);
        let data: Vec<u8> = (0..blocks).flat_map(|_| block(m)).collect();
        dev.write_texture(t, Subresource { mip: m, layer: 0 }, &data).unwrap();
    }
    t
}

const SRC: &str = r#"
struct VOut { float4 pos [[position]]; float2 uv; };
struct Empty { uint pad[16]; };
vertex VOut vs_full(uint vid [[vertex_id]], ED_BINDLESS(Empty)) {
    float2 p = float2((vid << 1) & 2, vid & 2) * 2.0 - 1.0;
    VOut o;
    o.pos = float4(p, 0.5, 1.0);
    o.uv = float2(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return o;
}
struct LodC { uint tex; uint smp; float lod; uint pad[13]; };
fragment float4 fs_lod(VOut in [[stage_in]], ED_BINDLESS(LodC)) {
    float4 c = ed_tex2d(ed_c.tex).sample(ed_smp(ed_c.smp), in.uv, level(ed_c.lod));
    return float4(c.rgb, 1.0);
}
"#;

/// Draws `t` at each mip level and returns the centre pixel per level.
fn sample_levels(dev: &dyn Device, t: TextureId, levels: u32) -> Vec<[u8; 4]> {
    let vs = shader(dev, ShaderStage::Vertex, "vs_full", SRC);
    let fs = shader(dev, ShaderStage::Fragment, "fs_lod", SRC);
    let pso = pipeline(dev, vs, fs, None);
    let ti = dev.bind_texture(t).unwrap();
    let smp = dev.bind_sampler(dev.create_sampler(&SamplerDesc { min_mag: Filter::Linear, mip: Filter::Nearest, address: AddressMode::Repeat, anisotropy: 1, compare: None }).unwrap()).unwrap();
    let rt = render_target(dev, "bc target");
    let rb = readback_buffer(dev, (Format::Rgba8Unorm.copy_row_pitch(W) * H) as u64);
    let mut out = Vec::new();
    for lod in 0..levels {
        let mut e = dev.begin_frame().unwrap();
        if lod == 0 {
            e.transition(&[tex(t, CopyDst, ShaderRead)]);
        }
        e.transition(&[tex(rt, Undefined, RenderTarget)]);
        e.begin_render_pass(&RenderPassDesc { label: "bc", colors: &[color(rt, LoadOp::Discard)], depth: None });
        e.set_render_pipeline(pso);
        let mut c = constants(&[ti.0, smp.0]);
        c[8..12].copy_from_slice(&(lod as f32).to_ne_bytes());
        e.set_draw_constants(&c);
        e.draw(0..3, 0..1);
        e.end_render_pass();
        copy_out(&mut *e, rt, rb);
        dev.end_frame(e).unwrap();
        let px = pixels(dev, rb);
        assert!(px.iter().all(|p| *p == px[0]), "level {lod}: solid texture must sample uniformly");
        out.push(at(&px, W / 2, H / 2));
    }
    out
}

fn close(a: [u8; 4], b: [u8; 4], tol: i32) -> bool {
    a.iter().zip(b).all(|(x, y)| (*x as i32 - y as i32).abs() <= tol)
}

#[test]
fn bc7_and_bc5_upload_with_mips_and_sample() {
    let dev = device();
    if !dev.caps().bc_textures {
        eprintln!("device has no BC texture support (Caps::bc_textures = false); skipping");
        return;
    }
    // BC7 Unorm: a different solid colour per mip, 16 x 16 -> 5 levels (down to 1 x 1, one block each).
    let cols7 = [[127, 0, 0, 127], [0, 127, 0, 127], [0, 0, 127, 127], [127, 127, 0, 127], [64, 32, 100, 127]];
    let t = bc_texture(&*dev, Format::Bc7Unorm, 16, 5, |m| bc7_solid(cols7[m as usize], 0));
    for (lod, got) in sample_levels(&*dev, t, 5).into_iter().enumerate() {
        let c = cols7[lod];
        let want = [(c[0] << 1) as u8, (c[1] << 1) as u8, (c[2] << 1) as u8, 255];
        assert!(close(got, want, 1), "BC7 level {lod}: got {got:?}, want {want:?}");
    }
    // BC7 sRGB: 254 encoded -> linear ~0.991 -> 253 in a Unorm target.
    let t = bc_texture(&*dev, Format::Bc7Srgb, 8, 1, |_| bc7_solid([127, 64, 0, 127], 0));
    let got = sample_levels(&*dev, t, 1)[0];
    let lin = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let want = [(lin(254.0 / 255.0) * 255.0).round() as u8, (lin(128.0 / 255.0) * 255.0).round() as u8, 0, 255];
    assert!(close(got, want, 2), "BC7 sRGB: got {got:?}, want {want:?}");
    // BC5: two channels, blue reads 0.
    let t = bc_texture(&*dev, Format::Bc5Unorm, 8, 2, |m| if m == 0 { bc5_solid(200, 100) } else { bc5_solid(30, 240) });
    let got = sample_levels(&*dev, t, 2);
    assert!(close(got[0], [200, 100, 0, 255], 1), "BC5 level 0: {:?}", got[0]);
    assert!(close(got[1], [30, 240, 0, 255], 1), "BC5 level 1: {:?}", got[1]);
}
