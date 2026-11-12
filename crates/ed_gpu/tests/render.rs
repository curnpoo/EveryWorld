//! Offscreen tests run on every compiled-in backend: clears, rasterisation, depth, compute,
//! bindless, indirect draws (CPU-written, compute-written and GPU-culled with a count),
//! indirect dispatch, instancing, mips and error reporting. Deferred destroy is in `alloc.rs`.

#[macro_use]
mod common;
use common::*;
use ed_gpu::ResourceState::*;
use ed_gpu::*;

backend_tests!(
    clear_and_exact_readback,
    triangle_covers_expected_pixels,
    reverse_z_nearer_wins_in_either_order,
    compute_writes_buffer,
    dispatch_indirect_uses_compute_written_group_count,
    bindless_draw_picks_textures_by_buffer_index,
    indirect_indexed_draw_with_compute_written_args,
    compute_cull_feeds_indirect_draws_with_count,
    instanced_draw_reads_bindless_instance_data,
    mip_upload_and_sample_chosen_level,
    foreign_shader_code_is_rejected,
    state_mismatch_is_reported_by_end_frame,
);

/// MSL side of `tri.vert`: triangle `ed_c.tri` (or the instance id when `ed_c.tri` is ~0) at
/// depth `ed_c.z`, flat colour `ed_c.col`.
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
"#;

fn tri_pipeline(dev: &dyn Device, depth: Option<DepthDesc>) -> RenderPipelineId {
    let vs = shader(dev, ShaderStage::Vertex, "vs_tri", TRI_SRC, spv!("tri.vert"));
    pipeline(dev, vs, flat_fs(dev), depth)
}

fn tri_constants(col: [f32; 4], z: f32, tri: u32) -> [u8; DRAW_CONSTANTS_BYTES] {
    let mut c = constants_f(&[col[0], col[1], col[2], col[3], z]);
    c[20..24].copy_from_slice(&tri.to_ne_bytes());
    c
}

const CLEAR: [f32; 4] = [51.0 / 255.0, 102.0 / 255.0, 204.0 / 255.0, 1.0];
const CLEAR_PX: [u8; 4] = [51, 102, 204, 255];

fn clear_and_exact_readback(dev: &dyn Device) {
    let rt = render_target(dev, "clear target");
    let rb = rt_readback(dev);
    let mut e = dev.begin_frame().unwrap();
    e.transition(&[tex(rt, Undefined, RenderTarget)]);
    e.begin_render_pass(&RenderPassDesc { label: "clear", colors: &[color(rt, LoadOp::Clear(CLEAR))], depth: None });
    e.end_render_pass();
    copy_out(&mut *e, rt, rb);
    dev.end_frame(e).unwrap();
    let px = pixels(dev, rb);
    assert!(px.iter().all(|p| *p == CLEAR_PX), "first pixel {:?}", px[0]);
}

fn triangle_covers_expected_pixels(dev: &dyn Device) {
    let pso = tri_pipeline(dev, None);
    let rt = render_target(dev, "triangle target");
    let rb = rt_readback(dev);
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
    let px = pixels(dev, rb);
    // Texture rows go top to bottom; clip-space Y is up.
    assert_eq!(at(&px, 2, H - 3), [255, 0, 0, 255], "bottom-left must be covered");
    assert_eq!(at(&px, W / 4, H * 3 / 4), [255, 0, 0, 255]);
    assert_eq!(at(&px, W - 3, 2), CLEAR_PX, "top-right must be uncovered (CW triangle culled)");
    assert_eq!(at(&px, W * 3 / 4, H / 4), CLEAR_PX);
    let covered = px.iter().filter(|p| **p == [255, 0, 0, 255]).count();
    let half = (W * H / 2) as usize;
    assert!(covered.abs_diff(half) <= W as usize, "covered {covered}, expected about {half}");
}

fn reverse_z_nearer_wins_in_either_order(dev: &dyn Device) {
    let pso = tri_pipeline(dev, Some(DepthDesc { format: Format::Depth32Float, write: true, compare: Compare::Greater, bias: 0.0, slope_bias: 0.0, clamp: false }));
    let rt = render_target(dev, "depth test target");
    let depth = depth_target(dev);
    let rb = rt_readback(dev);
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
        let px = pixels(dev, rb);
        assert!(px.iter().all(|p| *p == [255, 0, 0, 255]), "{order}: nearer (red) must win, got {:?}", px[(W * H / 2) as usize]);
    }
}

fn compute_writes_buffer(dev: &dyn Device) {
    const N: u32 = 4096;
    let cs = shader(dev, ShaderStage::Compute, "fill", FILL_MSL, spv!("fill.comp"));
    let pso = compute_pipeline(dev, cs, 64);
    let out = dev.create_buffer(&BufferDesc { label: "compute out", size: N as u64 * 4, usage: Usage { shader_write: true, copy_src: true, ..usage() } }).unwrap();
    let rb = readback_buffer(dev, N as u64 * 4);
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
    for (i, v) in words(dev, rb).into_iter().enumerate() {
        assert_eq!(v, i as u32 * 3 + 7, "element {i}");
    }
}

fn dispatch_indirect_uses_compute_written_group_count(dev: &dyn Device) {
    const N: u32 = 1024;
    let args_cs = shader(
        dev,
        ShaderStage::Compute,
        "dispatch_args",
        r#"
struct KC { uint args; uint groups; uint pad[14]; };
kernel void dispatch_args(uint gid [[thread_position_in_grid]], ED_BINDLESS(KC)) {
    device uint* a = ed_buf(uint, ed_c.args);
    a[0] = ed_c.groups; a[1] = 1; a[2] = 1;
}
"#,
        spv!("dispatch_args.comp"),
    );
    let args_pso = compute_pipeline(dev, args_cs, 1);
    let fill_cs = shader(dev, ShaderStage::Compute, "fill", FILL_MSL, spv!("fill.comp"));
    let fill_pso = compute_pipeline(dev, fill_cs, 64);
    let args = dev.create_buffer(&BufferDesc { label: "dispatch args", size: 12, usage: Usage { shader_write: true, indirect_args: true, ..usage() } }).unwrap();
    let out = dev.create_buffer(&BufferDesc { label: "fill out", size: N as u64 * 4, usage: Usage { shader_write: true, copy_src: true, copy_dst: true, ..usage() } }).unwrap();
    // Pre-fill with a marker: elements past the indirect group count must keep it.
    dev.write_buffer(out, 0, &bytes_u32(&vec![0xdead_beef; N as usize])).unwrap();
    let (ai, oi) = (dev.bind_buffer(args).unwrap(), dev.bind_buffer(out).unwrap());
    let rb = readback_buffer(dev, N as u64 * 4);
    let mut e = dev.begin_frame().unwrap();
    e.transition(&[buf(args, Undefined, ShaderWrite), buf(out, CopyDst, ShaderWrite)]);
    e.begin_compute_pass("write dispatch args");
    e.set_compute_pipeline(args_pso);
    e.set_compute_constants(&constants(&[ai.0, 5])); // 5 groups of 64 = 320 elements
    e.dispatch(1, 1, 1);
    e.end_compute_pass();
    e.transition(&[buf(args, ShaderWrite, IndirectArgs)]);
    e.begin_compute_pass("indirect fill");
    e.set_compute_pipeline(fill_pso);
    e.set_compute_constants(&constants(&[oi.0, N, 1]));
    e.dispatch_indirect(args, 0);
    e.end_compute_pass();
    e.transition(&[buf(out, ShaderWrite, CopySrc), buf(rb, Undefined, CopyDst)]);
    e.copy_buffer(out, 0, rb, 0, N as u64 * 4);
    dev.end_frame(e).unwrap();
    for (i, v) in words(dev, rb).into_iter().enumerate() {
        let want = if i < 320 { i as u32 * 3 + 1 } else { 0xdead_beef };
        assert_eq!(v, want, "element {i}");
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

fn full_vs(dev: &dyn Device) -> ShaderId {
    shader(dev, ShaderStage::Vertex, "vs_full", FULLSCREEN_VS, spv!("full.vert"))
}

fn bindless_draw_picks_textures_by_buffer_index(dev: &dyn Device) {
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
    let fs = shader(dev, ShaderStage::Fragment, "fs_sel", &fs_src, spv!("sel.frag"));
    let pso = pipeline(dev, full_vs(dev), fs, None);
    let red = solid_texture(dev, "red", 4, 1, &[[255, 0, 0, 255]]);
    let blue = solid_texture(dev, "blue", 4, 1, &[[0, 0, 255, 255]]);
    let (ri, bi) = (dev.bind_texture(red).unwrap(), dev.bind_texture(blue).unwrap());
    let smp = dev.bind_sampler(nearest_sampler(dev)).unwrap();
    let sel = dev.create_buffer(&BufferDesc { label: "selection", size: 8, usage: Usage { shader_read: true, copy_dst: true, ..usage() } }).unwrap();
    // Right half gets the red texture, left half blue: the table order decides, not binding order.
    dev.write_buffer(sel, 0, &bytes_u32(&[bi.0, ri.0])).unwrap();
    let seli = dev.bind_buffer(sel).unwrap();
    let rt = render_target(dev, "bindless target");
    let rb = rt_readback(dev);

    let mut e = dev.begin_frame().unwrap();
    e.transition(&[tex(red, CopyDst, ShaderRead), tex(blue, CopyDst, ShaderRead), buf(sel, CopyDst, ShaderRead), tex(rt, Undefined, RenderTarget)]);
    e.begin_render_pass(&RenderPassDesc { label: "bindless", colors: &[color(rt, LoadOp::Discard)], depth: None });
    e.set_render_pipeline(pso);
    e.set_draw_constants(&constants(&[seli.0, smp.0]));
    e.draw(0..3, 0..1);
    e.end_render_pass();
    copy_out(&mut *e, rt, rb);
    dev.end_frame(e).unwrap();
    let px = pixels(dev, rb);
    assert_eq!(at(&px, 3, H / 2), [0, 0, 255, 255], "left half samples table entry 0 (blue)");
    assert_eq!(at(&px, W - 4, H / 2), [255, 0, 0, 255], "right half samples table entry 1 (red)");
}

fn indirect_indexed_draw_with_compute_written_args(dev: &dyn Device) {
    let cs = shader(
        dev,
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
        spv!("write_args.comp"),
    );
    let cpso = compute_pipeline(dev, cs, 4);
    let pso = tri_pipeline(dev, None);
    let args = dev.create_buffer(&BufferDesc { label: "indirect args", size: 3 * 20, usage: Usage { shader_write: true, indirect_args: true, ..usage() } }).unwrap();
    let argi = dev.bind_buffer(args).unwrap();
    let ib = dev.create_buffer(&BufferDesc { label: "indices", size: 12, usage: Usage { shader_read: true, copy_dst: true, ..usage() } }).unwrap();
    dev.write_buffer(ib, 0, &bytes_u32(&[0, 1, 2])).unwrap();
    let rt = render_target(dev, "indirect target");
    let rb = rt_readback(dev);

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
    let px = pixels(dev, rb);
    assert_eq!(at(&px, 2, H - 3), [255, 0, 0, 255], "entry 0 drew triangle 0");
    assert_eq!(at(&px, W - 3, 2), [0, 255, 0, 255], "entry 1 drew triangle 1");
}

/// MSL side of `inst.vert`.
const INST_SRC: &str = r#"
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
"#;

/// Quadrants in clip space: 0 bottom-left red, 1 bottom-right green, 2 top-left blue,
/// 3 top-right white. Returns the instance buffer (in CopyDst) and its table index.
fn quadrant_instances(dev: &dyn Device) -> (BufferId, BindlessBuffer) {
    let quads: [([f32; 4], [f32; 4]); 4] = [
        ([-1.0, -1.0, 0.0, 0.0], [1.0, 0.0, 0.0, 1.0]),
        ([0.0, -1.0, 1.0, 0.0], [0.0, 1.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0]),
        ([0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]),
    ];
    let data: Vec<u8> = quads.iter().flat_map(|(r, c)| r.iter().chain(c.iter()).flat_map(|f| f.to_ne_bytes())).collect();
    let ib = dev.create_buffer(&BufferDesc { label: "instances", size: data.len() as u64, usage: Usage { shader_read: true, copy_dst: true, ..usage() } }).unwrap();
    dev.write_buffer(ib, 0, &data).unwrap();
    (ib, dev.bind_buffer(ib).unwrap())
}

const QUAD_COLORS: [[u8; 4]; 4] = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 255, 255]];

/// Centre pixels of quadrants 0..4 (bottom-left, bottom-right, top-left, top-right).
fn quadrant_pixels(px: &[[u8; 4]]) -> [[u8; 4]; 4] {
    let (l, r, t, b) = (W / 4, W * 3 / 4, H / 4, H * 3 / 4);
    [at(px, l, b), at(px, r, b), at(px, l, t), at(px, r, t)]
}

fn instanced_draw_reads_bindless_instance_data(dev: &dyn Device) {
    let vs = shader(dev, ShaderStage::Vertex, "vs_inst", INST_SRC, spv!("inst.vert"));
    let pso = pipeline(dev, vs, flat_fs(dev), None);
    let (ib, ii) = quadrant_instances(dev);
    let rt = render_target(dev, "instancing target");
    let rb = rt_readback(dev);

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
    let q = quadrant_pixels(&pixels(dev, rb));
    assert_eq!(q[0], CLEAR_PX, "instance 0 not drawn");
    assert_eq!(q[1..], QUAD_COLORS[1..], "instances 1..4");
}

/// GPU culling: a compute pass appends one indexed draw per visible object with an atomic
/// counter, then zeroes the unused tail; the draw takes the count buffer. Objects 0, 2, 3 are
/// visible, so quadrant 1 must stay clear. Works whether or not `Caps::indirect_count` is set.
fn compute_cull_feeds_indirect_draws_with_count(dev: &dyn Device) {
    const N: u32 = 4;
    let cs = shader(
        dev,
        ShaderStage::Compute,
        "cull",
        r#"
struct Args { uint index_count; uint instance_count; uint first_index; int base_vertex; uint first_instance; };
struct KC { uint objs; uint args; uint count; uint n; uint mode; uint pad[11]; };
kernel void cull(uint gid [[thread_position_in_grid]], ED_BINDLESS(KC)) {
    device atomic_uint* count = reinterpret_cast<device atomic_uint*>(ed_buf(uint, ed_c.count));
    device Args* args = ed_buf(Args, ed_c.args);
    if (ed_c.mode == 0) {
        if (gid == 0) { atomic_store_explicit(count, 0u, memory_order_relaxed); }
    } else if (ed_c.mode == 1) {
        if (gid < ed_c.n && ed_buf(uint, ed_c.objs)[gid] != 0) {
            uint slot = atomic_fetch_add_explicit(count, 1u, memory_order_relaxed);
            args[slot] = Args { 6, 1, 0, 0, gid };
        }
    } else {
        if (gid < ed_c.n && gid >= atomic_load_explicit(count, memory_order_relaxed)) { args[gid] = Args { 0, 0, 0, 0, 0 }; }
    }
}
"#,
        spv!("cull.comp"),
    );
    let cpso = compute_pipeline(dev, cs, 64);
    let vs = shader(dev, ShaderStage::Vertex, "vs_inst", INST_SRC, spv!("inst.vert"));
    let pso = pipeline(dev, vs, flat_fs(dev), None);
    let (inst, insti) = quadrant_instances(dev);
    let objs = dev.create_buffer(&BufferDesc { label: "visibility", size: N as u64 * 4, usage: Usage { shader_read: true, copy_dst: true, ..usage() } }).unwrap();
    dev.write_buffer(objs, 0, &bytes_u32(&[1, 0, 1, 1])).unwrap();
    let indices = dev.create_buffer(&BufferDesc { label: "quad indices", size: 24, usage: Usage { shader_read: true, copy_dst: true, ..usage() } }).unwrap();
    dev.write_buffer(indices, 0, &bytes_u32(&[0, 1, 2, 3, 4, 5])).unwrap();
    let args = dev.create_buffer(&BufferDesc { label: "culled args", size: N as u64 * 20, usage: Usage { shader_write: true, indirect_args: true, ..usage() } }).unwrap();
    let count = dev.create_buffer(&BufferDesc { label: "draw count", size: 4, usage: Usage { shader_write: true, indirect_args: true, copy_src: true, ..usage() } }).unwrap();
    let (oi, ai, ci) = (dev.bind_buffer(objs).unwrap(), dev.bind_buffer(args).unwrap(), dev.bind_buffer(count).unwrap());
    let rt = render_target(dev, "cull target");
    let rb = rt_readback(dev);
    let count_rb = readback_buffer(dev, 4);

    let mut e = dev.begin_frame().unwrap();
    e.transition(&[buf(objs, CopyDst, ShaderRead), buf(args, Undefined, ShaderWrite), buf(count, Undefined, ShaderWrite)]);
    for mode in 0..3u32 {
        if mode > 0 {
            e.transition(&[buf(args, ShaderWrite, ShaderWrite), buf(count, ShaderWrite, ShaderWrite)]);
        }
        e.begin_compute_pass("cull");
        e.set_compute_pipeline(cpso);
        e.set_compute_constants(&constants(&[oi.0, ai.0, ci.0, N, mode]));
        e.dispatch(1, 1, 1);
        e.end_compute_pass();
    }
    e.transition(&[
        buf(args, ShaderWrite, IndirectArgs),
        buf(count, ShaderWrite, IndirectArgs),
        buf(inst, CopyDst, ShaderRead),
        buf(indices, CopyDst, ShaderRead),
        tex(rt, Undefined, RenderTarget),
    ]);
    e.begin_render_pass(&RenderPassDesc { label: "culled draws", colors: &[color(rt, LoadOp::Clear(CLEAR))], depth: None });
    e.set_render_pipeline(pso);
    e.set_index_buffer(indices, 0);
    e.set_draw_constants(&constants(&[insti.0]));
    e.draw_indexed_indirect_multi(args, 0, N, Some((count, 0)));
    e.end_render_pass();
    copy_out(&mut *e, rt, rb);
    e.transition(&[buf(count, IndirectArgs, CopySrc), buf(count_rb, Undefined, CopyDst)]);
    e.copy_buffer(count, 0, count_rb, 0, 4);
    dev.end_frame(e).unwrap();
    let q = quadrant_pixels(&pixels(dev, rb));
    assert_eq!(words(dev, count_rb), [3], "three visible objects");
    assert_eq!([q[0], q[2], q[3]], [QUAD_COLORS[0], QUAD_COLORS[2], QUAD_COLORS[3]], "visible objects drawn");
    assert_eq!(q[1], CLEAR_PX, "culled object 1 not drawn");
}

fn mip_upload_and_sample_chosen_level(dev: &dyn Device) {
    let src = format!(
        "{FULLSCREEN_VS}{}",
        r#"
struct LodC { uint tex; uint smp; float lod; uint pad[13]; };
fragment float4 fs_lod(VOut in [[stage_in]], ED_BINDLESS(LodC)) {
    return ed_tex2d(ed_c.tex).sample(ed_smp(ed_c.smp), in.uv, level(ed_c.lod));
}
"#
    );
    let fs = shader(dev, ShaderStage::Fragment, "fs_lod", &src, spv!("lod.frag"));
    let pso = pipeline(dev, full_vs(dev), fs, None);
    let colors = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 0, 255]];
    let t = solid_texture(dev, "mipped", 8, 4, &colors);
    let ti = dev.bind_texture(t).unwrap();
    let smp = dev.bind_sampler(nearest_sampler(dev)).unwrap();
    let rt = render_target(dev, "mip target");
    let rb = rt_readback(dev);
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
        let px = pixels(dev, rb);
        assert!(px.iter().all(|p| p == want), "level {lod}: want {want:?}, got {:?}", px[0]);
    }
}

fn foreign_shader_code_is_rejected(dev: &dyn Device) {
    let rejected = |code: ShaderCode<'_>| {
        let e = dev.create_shader(&ShaderDesc { label: "foreign", stage: ShaderStage::Vertex, entry: "main", code }).unwrap_err();
        assert!(matches!(e, Error::Unsupported(_) | Error::Invalid(_)), "{e}");
    };
    rejected(ShaderCode::Dxil(&[0; 4]));
    match dev.caps().backend {
        Backend::Vulkan => {
            rejected(ShaderCode::MslSource("kernel void main() {}"));
            rejected(ShaderCode::SpirV(&[0x0723_0203]));
            // Valid SPIR-V, but the wrong entry point or stage.
            let words = spirv_words(spv!("fill.comp"));
            let e = dev.create_shader(&ShaderDesc { label: "no entry", stage: ShaderStage::Compute, entry: "nope", code: ShaderCode::SpirV(&words) }).unwrap_err();
            assert!(matches!(e, Error::Invalid(_)), "{e}");
            let e = dev.create_shader(&ShaderDesc { label: "wrong stage", stage: ShaderStage::Vertex, entry: "main", code: ShaderCode::SpirV(&words) }).unwrap_err();
            assert!(matches!(e, Error::Invalid(_)), "{e}");
        }
        _ => rejected(ShaderCode::SpirV(&[0x0723_0203])),
    }
}

fn state_mismatch_is_reported_by_end_frame(dev: &dyn Device) {
    let rt = render_target(dev, "misused");
    let mut e = dev.begin_frame().unwrap();
    // Never transitioned to RenderTarget.
    e.begin_render_pass(&RenderPassDesc { label: "bad", colors: &[color(rt, LoadOp::Clear(CLEAR))], depth: None });
    e.end_render_pass();
    assert!(dev.end_frame(e).is_err());
    // The next frame is clean again.
    let e = dev.begin_frame().unwrap();
    dev.end_frame(e).unwrap();
}
