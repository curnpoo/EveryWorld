//! Offscreen Metal tests: clears, rasterisation, depth, compute, bindless, indirect draws,
//! instancing and mips. Compiles to nothing off macOS.
#![cfg(target_os = "macos")]

mod common;
use common::*;
use ed_gpu::ResourceState::*;
use ed_gpu::*;

/// Vertex shader for the triangle tests: triangle `ed_c.tri` (or the instance id when
/// `ed_c.tri` is ~0) at depth `ed_c.z`, flat colour `ed_c.col`.
const TRI_SRC: &str = r#"
struct VOut { float4 pos [[position]]; float4 col [[flat]]; float2 uv; };
struct TriC { float4 col; float z; uint tri; uint pad[10]; };
constant float2 TRIS[4][3] = {
    { float2(-1, -1), float2(1, -1), float2(-1, 1) },   // 0: bottom-left half, CCW
    { float2(1, 1), float2(-1, 1), float2(1, -1) },     // 1: top-right half, CCW
    { float2(1, 1), float2(1, -1), float2(-1, 1) },     // 2: top-right half, CW
    { float2(-3, -1), float2(3, -1), float2(0, 3) },    // 3: full screen, CCW
};
vertex VOut vs_tri(uint vid [[vertex_id]], uint iid [[instance_id]], ED_BINDLESS(TriC)) {
    uint t = ed_c.tri == 0xffffffffu ? iid : ed_c.tri;
    VOut o;
    o.pos = float4(TRIS[t][vid % 3], ed_c.z, 1.0);
    o.col = ed_c.tri == 0xffffffffu ? (iid == 0 ? float4(1, 0, 0, 1) : float4(0, 1, 0, 1)) : ed_c.col;
    o.uv = float2(0);
    return o;
}
struct Empty { uint pad[16]; };
fragment float4 fs_flat(VOut in [[stage_in]], ED_BINDLESS(Empty)) { return in.col; }
"#;

fn tri_constants(col: [f32; 4], z: f32, tri: u32) -> [u8; DRAW_CONSTANTS_BYTES] {
    let mut c = constants_f(&[col[0], col[1], col[2], col[3], z]);
    c[20..24].copy_from_slice(&tri.to_ne_bytes());
    c
}

const CLEAR: [f32; 4] = [51.0 / 255.0, 102.0 / 255.0, 204.0 / 255.0, 1.0];
const CLEAR_PX: [u8; 4] = [51, 102, 204, 255];

#[test]
fn clear_and_exact_readback() {
    let dev = device();
    let rt = render_target(&*dev, "clear target");
    let rb = readback_buffer(&*dev, (Format::Rgba8Unorm.copy_row_pitch(W) * H) as u64);
    let mut e = dev.begin_frame().unwrap();
    e.transition(&[tex(rt, Undefined, RenderTarget)]);
    e.begin_render_pass(&RenderPassDesc { label: "clear", colors: &[color(rt, LoadOp::Clear(CLEAR))], depth: None });
    e.end_render_pass();
    copy_out(&mut *e, rt, rb);
    dev.end_frame(e).unwrap();
    let px = pixels(&*dev, rb);
    assert!(px.iter().all(|p| *p == CLEAR_PX), "first pixel {:?}", px[0]);
}

#[test]
fn triangle_covers_expected_pixels() {
    let dev = device();
    let vs = shader(&*dev, ShaderStage::Vertex, "vs_tri", TRI_SRC);
    let fs = shader(&*dev, ShaderStage::Fragment, "fs_flat", TRI_SRC);
    let pso = pipeline(&*dev, vs, fs, None);
    let rt = render_target(&*dev, "triangle target");
    let rb = readback_buffer(&*dev, (Format::Rgba8Unorm.copy_row_pitch(W) * H) as u64);
    let mut e = dev.begin_frame().unwrap();
    e.transition(&[tex(rt, Undefined, RenderTarget)]);
    e.begin_render_pass(&RenderPassDesc { label: "triangle", colors: &[color(rt, LoadOp::Clear(CLEAR))], depth: None });
    e.set_render_pipeline(pso);
    e.set_viewport(0.0, 0.0, W as f32, H as f32);
    // CCW bottom-left triangle: drawn. CW top-right triangle: culled as a back face.
    e.set_draw_constants(&tri_constants([1.0, 0.0, 0.0, 1.0], 0.5, 0));
    e.draw(0..3, 0..1);
    e.set_draw_constants(&tri_constants([0.0, 1.0, 0.0, 1.0], 0.5, 2));
    e.draw(0..3, 0..1);
    e.end_render_pass();
    copy_out(&mut *e, rt, rb);
    dev.end_frame(e).unwrap();
    let px = pixels(&*dev, rb);
    // Texture rows go top to bottom; clip-space Y is up.
    assert_eq!(at(&px, 2, H - 3), [255, 0, 0, 255], "bottom-left must be covered");
    assert_eq!(at(&px, W / 4, H * 3 / 4), [255, 0, 0, 255]);
    assert_eq!(at(&px, W - 3, 2), CLEAR_PX, "top-right must be uncovered (CW triangle culled)");
    assert_eq!(at(&px, W * 3 / 4, H / 4), CLEAR_PX);
    let covered = px.iter().filter(|p| **p == [255, 0, 0, 255]).count();
    let half = (W * H / 2) as usize;
    assert!(covered.abs_diff(half) <= W as usize, "covered {covered}, expected about {half}");
}

#[test]
fn reverse_z_nearer_wins_in_either_order() {
    let dev = device();
    let vs = shader(&*dev, ShaderStage::Vertex, "vs_tri", TRI_SRC);
    let fs = shader(&*dev, ShaderStage::Fragment, "fs_flat", TRI_SRC);
    let pso = pipeline(&*dev, vs, fs, Some(DepthDesc { format: Format::Depth32Float, write: true, compare: Compare::Greater, bias: 0.0, slope_bias: 0.0 }));
    let rt = render_target(&*dev, "depth test target");
    let depth = depth_target(&*dev);
    let rb = readback_buffer(&*dev, (Format::Rgba8Unorm.copy_row_pitch(W) * H) as u64);
    let near = ([1.0, 0.0, 0.0, 1.0], 0.8); // reverse-Z: larger is nearer
    let far = ([0.0, 1.0, 0.0, 1.0], 0.2);
    for (order, first) in [("near first", [near, far]), ("far first", [far, near])] {
        let mut e = dev.begin_frame().unwrap();
        e.transition(&[tex(rt, Undefined, RenderTarget), tex(depth, Undefined, DepthWrite)]);
        e.begin_render_pass(&RenderPassDesc {
            label: order,
            colors: &[color(rt, LoadOp::Clear(CLEAR))],
            depth: Some(Attachment { texture: depth, sub: SUB0, load: LoadOp::Clear([0.0; 4]), store: StoreOp::Discard }),
        });
        e.set_render_pipeline(pso);
        for (col, z) in first {
            e.set_draw_constants(&tri_constants(col, z, 3));
            e.draw(0..3, 0..1);
        }
        e.end_render_pass();
        copy_out(&mut *e, rt, rb);
        dev.end_frame(e).unwrap();
        let px = pixels(&*dev, rb);
        assert!(px.iter().all(|p| *p == [255, 0, 0, 255]), "{order}: nearer (red) must win, got {:?}", px[(W * H / 2) as usize]);
    }
}

#[test]
fn compute_writes_buffer() {
    let dev = device();
    const N: u32 = 4096;
    let cs = shader(
        &*dev,
        ShaderStage::Compute,
        "fill",
        r#"
struct KC { uint buf; uint n; uint add; uint pad[13]; };
kernel void fill(uint gid [[thread_position_in_grid]], ED_BINDLESS(KC)) {
    if (gid < ed_c.n) { ed_buf(uint, ed_c.buf)[gid] = gid * 3u + ed_c.add; }
}
"#,
    );
    let pso = compute_pipeline(&*dev, cs, 64);
    let out = dev.create_buffer(&BufferDesc { label: "compute out", size: N as u64 * 4, usage: Usage { shader_write: true, copy_src: true, ..usage() } }).unwrap();
    let rb = readback_buffer(&*dev, N as u64 * 4);
    let idx = dev.bind_buffer(out).unwrap();
    let mut e = dev.begin_frame().unwrap();
    e.transition(&[buf(out, Undefined, ShaderWrite)]);
    e.begin_compute_pass("fill");
    e.set_compute_pipeline(pso);
    e.set_compute_constants(&constants(&[idx.0, N, 7]));
    e.dispatch(N / 64, 1, 1);
    e.end_compute_pass();
    e.transition(&[buf(out, ShaderWrite, CopySrc), buf(rb, Undefined, CopyDst)]);
    e.copy_buffer(out, 0, rb, 0, N as u64 * 4);
    dev.end_frame(e).unwrap();
    dev.wait_idle();
    let mut bytes = Vec::new();
    dev.read_buffer(rb, &mut bytes).unwrap();
    for i in 0..N as usize {
        let v = u32::from_ne_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
        assert_eq!(v, i as u32 * 3 + 7, "element {i}");
    }
}

fn solid_texture(dev: &dyn Device, label: &str, size: u32, mips: u32, colors: &[[u8; 4]]) -> TextureId {
    let t = dev
        .create_texture(&TextureDesc {
            label,
            kind: TextureKind::D2,
            format: Format::Rgba8Unorm,
            width: size,
            height: size,
            depth_or_layers: 1,
            mips,
            samples: 1,
            usage: Usage { shader_read: true, copy_dst: true, ..usage() },
        })
        .unwrap();
    for m in 0..mips {
        let s = mip_size(size, m);
        let data: Vec<u8> = (0..s * s).flat_map(|_| colors[m as usize]).collect();
        dev.write_texture(t, Subresource { mip: m, layer: 0 }, &data).unwrap();
    }
    t
}

fn nearest_sampler(dev: &dyn Device) -> SamplerId {
    dev.create_sampler(&SamplerDesc { min_mag: Filter::Nearest, mip: Filter::Nearest, address: AddressMode::Clamp, anisotropy: 1, compare: None }).unwrap()
}

/// Full-screen triangle with uv in 0..1 (v down); fragment code supplied per test.
const FULLSCREEN_VS: &str = r#"
struct VOut { float4 pos [[position]]; float4 col [[flat]]; float2 uv; };
struct Empty { uint pad[16]; };
vertex VOut vs_full(uint vid [[vertex_id]], ED_BINDLESS(Empty)) {
    float2 p = float2((vid << 1) & 2, vid & 2) * 2.0 - 1.0; // (-1,-1) (3,-1) (-1,3): CCW
    VOut o;
    o.pos = float4(p, 0.5, 1.0);
    o.col = float4(0);
    o.uv = float2(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return o;
}
"#;

#[test]
fn bindless_draw_picks_textures_by_buffer_index() {
    let dev = device();
    let fs_src = format!(
        "{FULLSCREEN_VS}{}",
        r#"
struct SelC { uint sel; uint smp; uint pad[14]; };
fragment float4 fs_sel(VOut in [[stage_in]], ED_BINDLESS(SelC)) {
    device uint* ids = ed_buf(uint, ed_c.sel);
    uint t = in.uv.x < 0.5 ? ids[0] : ids[1];
    return ed_tex2d(t).sample(ed_smp(ed_c.smp), in.uv);
}
"#
    );
    let vs = shader(&*dev, ShaderStage::Vertex, "vs_full", &fs_src);
    let fs = shader(&*dev, ShaderStage::Fragment, "fs_sel", &fs_src);
    let pso = pipeline(&*dev, vs, fs, None);
    let red = solid_texture(&*dev, "red", 4, 1, &[[255, 0, 0, 255]]);
    let blue = solid_texture(&*dev, "blue", 4, 1, &[[0, 0, 255, 255]]);
    let (ri, bi) = (dev.bind_texture(red).unwrap(), dev.bind_texture(blue).unwrap());
    let smp = dev.bind_sampler(nearest_sampler(&*dev)).unwrap();
    let sel = dev.create_buffer(&BufferDesc { label: "selection", size: 8, usage: Usage { shader_read: true, copy_dst: true, ..usage() } }).unwrap();
    // Right half gets the red texture, left half blue: the table order decides, not binding order.
    let ids: Vec<u8> = [bi.0, ri.0].iter().flat_map(|v| v.to_ne_bytes()).collect();
    dev.write_buffer(sel, 0, &ids).unwrap();
    let seli = dev.bind_buffer(sel).unwrap();
    let rt = render_target(&*dev, "bindless target");
    let rb = readback_buffer(&*dev, (Format::Rgba8Unorm.copy_row_pitch(W) * H) as u64);

    let mut e = dev.begin_frame().unwrap();
    e.transition(&[tex(red, CopyDst, ShaderRead), tex(blue, CopyDst, ShaderRead), buf(sel, CopyDst, ShaderRead), tex(rt, Undefined, RenderTarget)]);
    e.begin_render_pass(&RenderPassDesc { label: "bindless", colors: &[color(rt, LoadOp::Discard)], depth: None });
    e.set_render_pipeline(pso);
    e.set_draw_constants(&constants(&[seli.0, smp.0]));
    e.draw(0..3, 0..1);
    e.end_render_pass();
    copy_out(&mut *e, rt, rb);
    dev.end_frame(e).unwrap();
    let px = pixels(&*dev, rb);
    assert_eq!(at(&px, 3, H / 2), [0, 0, 255, 255], "left half samples table entry 0 (blue)");
    assert_eq!(at(&px, W - 4, H / 2), [255, 0, 0, 255], "right half samples table entry 1 (red)");
}

#[test]
fn indirect_indexed_draw_with_compute_written_args() {
    let dev = device();
    let cs = shader(
        &*dev,
        ShaderStage::Compute,
        "write_args",
        r#"
struct Args { uint index_count; uint instance_count; uint first_index; int base_vertex; uint first_instance; };
struct KC { uint args; uint pad[15]; };
kernel void write_args(uint gid [[thread_position_in_grid]], ED_BINDLESS(KC)) {
    device Args* a = ed_buf(Args, ed_c.args);
    if (gid == 0) { a[0] = Args { 3, 1, 0, 0, 0 }; }   // triangle 0, red
    if (gid == 1) { a[1] = Args { 3, 1, 0, 0, 1 }; }   // triangle 1, green
    if (gid == 2) { a[2] = Args { 0, 0, 0, 0, 0 }; }   // empty entry
}
"#,
    );
    let cpso = compute_pipeline(&*dev, cs, 4);
    let vs = shader(&*dev, ShaderStage::Vertex, "vs_tri", TRI_SRC);
    let fs = shader(&*dev, ShaderStage::Fragment, "fs_flat", TRI_SRC);
    let pso = pipeline(&*dev, vs, fs, None);
    let args = dev.create_buffer(&BufferDesc { label: "indirect args", size: 3 * 20, usage: Usage { shader_write: true, indirect_args: true, ..usage() } }).unwrap();
    let argi = dev.bind_buffer(args).unwrap();
    let ib = dev.create_buffer(&BufferDesc { label: "indices", size: 12, usage: Usage { shader_read: true, copy_dst: true, ..usage() } }).unwrap();
    dev.write_buffer(ib, 0, &[0u32, 1, 2].iter().flat_map(|v| v.to_ne_bytes()).collect::<Vec<_>>()).unwrap();
    let rt = render_target(&*dev, "indirect target");
    let rb = readback_buffer(&*dev, (Format::Rgba8Unorm.copy_row_pitch(W) * H) as u64);

    let mut e = dev.begin_frame().unwrap();
    e.transition(&[buf(args, Undefined, ShaderWrite)]);
    e.begin_compute_pass("write args");
    e.set_compute_pipeline(cpso);
    e.set_compute_constants(&constants(&[argi.0]));
    e.dispatch(1, 1, 1);
    e.end_compute_pass();
    e.transition(&[buf(args, ShaderWrite, IndirectArgs), buf(ib, CopyDst, ShaderRead), tex(rt, Undefined, RenderTarget)]);
    e.begin_render_pass(&RenderPassDesc { label: "indirect", colors: &[color(rt, LoadOp::Clear(CLEAR))], depth: None });
    e.set_render_pipeline(pso);
    e.set_index_buffer(ib, 0);
    e.set_draw_constants(&tri_constants([0.0; 4], 0.5, u32::MAX));
    e.draw_indexed_indirect_multi(args, 0, 3, None);
    e.end_render_pass();
    copy_out(&mut *e, rt, rb);
    dev.end_frame(e).unwrap();
    let px = pixels(&*dev, rb);
    assert_eq!(at(&px, 2, H - 3), [255, 0, 0, 255], "entry 0 drew triangle 0");
    assert_eq!(at(&px, W - 3, 2), [0, 255, 0, 255], "entry 1 drew triangle 1");
}

#[test]
fn instanced_draw_reads_bindless_instance_data() {
    let dev = device();
    let src = r#"
struct VOut { float4 pos [[position]]; float4 col [[flat]]; float2 uv; };
struct Inst { float4 rect; float4 col; };
struct IC { uint inst; uint pad[15]; };
constant float2 Q[6] = { float2(0,0), float2(1,0), float2(0,1), float2(0,1), float2(1,0), float2(1,1) };
vertex VOut vs_inst(uint vid [[vertex_id]], uint iid [[instance_id]], ED_BINDLESS(IC)) {
    Inst d = ed_buf(Inst, ed_c.inst)[iid];
    float2 q = Q[vid];
    VOut o;
    o.pos = float4(mix(d.rect.xy, d.rect.zw, q), 0.5, 1.0);
    o.col = d.col;
    o.uv = q;
    return o;
}
struct Empty { uint pad[16]; };
fragment float4 fs_flat(VOut in [[stage_in]], ED_BINDLESS(Empty)) { return in.col; }
"#;
    let vs = shader(&*dev, ShaderStage::Vertex, "vs_inst", src);
    let fs = shader(&*dev, ShaderStage::Fragment, "fs_flat", src);
    let pso = pipeline(&*dev, vs, fs, None);
    // Quadrants in clip space: 0 bottom-left, 1 bottom-right, 2 top-left, 3 top-right.
    let quads: [([f32; 4], [f32; 4]); 4] = [
        ([-1.0, -1.0, 0.0, 0.0], [1.0, 0.0, 0.0, 1.0]),
        ([0.0, -1.0, 1.0, 0.0], [0.0, 1.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0]),
        ([0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]),
    ];
    let data: Vec<u8> = quads.iter().flat_map(|(r, c)| r.iter().chain(c.iter()).flat_map(|f| f.to_ne_bytes())).collect();
    let ib = dev.create_buffer(&BufferDesc { label: "instances", size: data.len() as u64, usage: Usage { shader_read: true, copy_dst: true, ..usage() } }).unwrap();
    dev.write_buffer(ib, 0, &data).unwrap();
    let ii = dev.bind_buffer(ib).unwrap();
    let rt = render_target(&*dev, "instancing target");
    let rb = readback_buffer(&*dev, (Format::Rgba8Unorm.copy_row_pitch(W) * H) as u64);

    let mut e = dev.begin_frame().unwrap();
    e.transition(&[buf(ib, CopyDst, ShaderRead), tex(rt, Undefined, RenderTarget)]);
    e.begin_render_pass(&RenderPassDesc { label: "instanced", colors: &[color(rt, LoadOp::Clear(CLEAR))], depth: None });
    e.set_render_pipeline(pso);
    e.set_draw_constants(&constants(&[ii.0]));
    // Instances 1..4: quadrant 0 stays clear, which also checks the base instance.
    e.draw(0..6, 1..4);
    e.end_render_pass();
    copy_out(&mut *e, rt, rb);
    dev.end_frame(e).unwrap();
    let px = pixels(&*dev, rb);
    let (l, r, t, b) = (W / 4, W * 3 / 4, H / 4, H * 3 / 4);
    assert_eq!(at(&px, l, b), CLEAR_PX, "instance 0 not drawn");
    assert_eq!(at(&px, r, b), [0, 255, 0, 255], "instance 1");
    assert_eq!(at(&px, l, t), [0, 0, 255, 255], "instance 2");
    assert_eq!(at(&px, r, t), [255, 255, 255, 255], "instance 3");
}

#[test]
fn mip_upload_and_sample_chosen_level() {
    let dev = device();
    let src = format!(
        "{FULLSCREEN_VS}{}",
        r#"
struct LodC { uint tex; uint smp; float lod; uint pad[13]; };
fragment float4 fs_lod(VOut in [[stage_in]], ED_BINDLESS(LodC)) {
    return ed_tex2d(ed_c.tex).sample(ed_smp(ed_c.smp), in.uv, level(ed_c.lod));
}
"#
    );
    let vs = shader(&*dev, ShaderStage::Vertex, "vs_full", &src);
    let fs = shader(&*dev, ShaderStage::Fragment, "fs_lod", &src);
    let pso = pipeline(&*dev, vs, fs, None);
    let colors = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 0, 255]];
    let t = solid_texture(&*dev, "mipped", 8, 4, &colors);
    let ti = dev.bind_texture(t).unwrap();
    let smp = dev.bind_sampler(nearest_sampler(&*dev)).unwrap();
    let rt = render_target(&*dev, "mip target");
    let rb = readback_buffer(&*dev, (Format::Rgba8Unorm.copy_row_pitch(W) * H) as u64);
    let mut first = true;
    for (lod, want) in colors.iter().enumerate() {
        let mut e = dev.begin_frame().unwrap();
        if first {
            e.transition(&[tex(t, CopyDst, ShaderRead)]);
            first = false;
        }
        e.transition(&[tex(rt, Undefined, RenderTarget)]);
        e.begin_render_pass(&RenderPassDesc { label: "mip", colors: &[color(rt, LoadOp::Discard)], depth: None });
        e.set_render_pipeline(pso);
        let mut c = constants(&[ti.0, smp.0]);
        c[8..12].copy_from_slice(&(lod as f32).to_ne_bytes());
        e.set_draw_constants(&c);
        e.draw(0..3, 0..1);
        e.end_render_pass();
        copy_out(&mut *e, rt, rb);
        dev.end_frame(e).unwrap();
        let px = pixels(&*dev, rb);
        assert!(px.iter().all(|p| p == want), "level {lod}: want {want:?}, got {:?}", px[0]);
    }
}

#[test]
fn foreign_shader_code_is_rejected() {
    let dev = device();
    let e = dev.create_shader(&ShaderDesc { label: "spirv", stage: ShaderStage::Vertex, entry: "main", code: ShaderCode::SpirV(&[0x0723_0203]) }).unwrap_err();
    assert!(matches!(e, Error::Unsupported(_) | Error::Invalid(_)), "{e}");
    let e = dev.create_shader(&ShaderDesc { label: "dxil", stage: ShaderStage::Vertex, entry: "main", code: ShaderCode::Dxil(&[0; 4]) }).unwrap_err();
    assert!(matches!(e, Error::Unsupported(_) | Error::Invalid(_)), "{e}");
}

#[test]
fn state_mismatch_is_reported_by_end_frame() {
    let dev = device();
    let rt = render_target(&*dev, "misused");
    let mut e = dev.begin_frame().unwrap();
    // Never transitioned to RenderTarget.
    e.begin_render_pass(&RenderPassDesc { label: "bad", colors: &[color(rt, LoadOp::Clear(CLEAR))], depth: None });
    e.end_render_pass();
    assert!(dev.end_frame(e).is_err());
    // The next frame is clean again.
    let e = dev.begin_frame().unwrap();
    dev.end_frame(e).unwrap();
}
