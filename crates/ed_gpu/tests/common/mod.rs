//! Shared helpers for the offscreen Metal tests.
#![allow(dead_code)]

use ed_gpu::*;

pub const W: u32 = 64;
pub const H: u32 = 64;

pub fn device() -> Box<dyn Device> {
    create_device(Backend::Metal, &DeviceDesc { validation: true, frames_in_flight: 2 }).expect("Metal device")
}

pub fn usage() -> Usage {
    Usage::default()
}

pub fn shader(dev: &dyn Device, stage: ShaderStage, entry: &str, body: &str) -> ShaderId {
    let src = format!("{}\n{}", shader_abi::MSL_PRELUDE, body);
    dev.create_shader(&ShaderDesc { label: entry, stage, entry, code: ShaderCode::MslSource(&src) }).unwrap_or_else(|e| panic!("shader {entry}: {e}"))
}

pub fn render_target(dev: &dyn Device, label: &str) -> TextureId {
    dev.create_texture(&TextureDesc {
        label,
        kind: TextureKind::D2,
        format: Format::Rgba8Unorm,
        width: W,
        height: H,
        depth_or_layers: 1,
        mips: 1,
        samples: 1,
        usage: Usage { render_target: true, copy_src: true, ..usage() },
    })
    .expect("render target")
}

pub fn depth_target(dev: &dyn Device) -> TextureId {
    dev.create_texture(&TextureDesc {
        label: "depth",
        kind: TextureKind::D2,
        format: Format::Depth32Float,
        width: W,
        height: H,
        depth_or_layers: 1,
        mips: 1,
        samples: 1,
        usage: Usage { depth: true, ..usage() },
    })
    .expect("depth target")
}

pub fn readback_buffer(dev: &dyn Device, size: u64) -> BufferId {
    dev.create_buffer(&BufferDesc { label: "readback", size, usage: Usage { copy_dst: true, cpu_read: true, ..usage() } }).expect("readback buffer")
}

pub const SUB0: Subresource = Subresource { mip: 0, layer: 0 };

pub fn tex(texture: TextureId, from: ResourceState, to: ResourceState) -> Transition {
    Transition::Texture { texture, sub: None, from, to }
}

pub fn buf(buffer: BufferId, from: ResourceState, to: ResourceState) -> Transition {
    Transition::Buffer { buffer, from, to }
}

pub fn color(texture: TextureId, load: LoadOp) -> Attachment {
    Attachment { texture, sub: SUB0, load, store: StoreOp::Store }
}

/// Records a copy of the RGBA8 render target into `rb` (state RenderTarget -> CopySrc).
pub fn copy_out(e: &mut dyn CommandEncoder, rt: TextureId, rb: BufferId) {
    e.transition(&[tex(rt, ResourceState::RenderTarget, ResourceState::CopySrc), buf(rb, ResourceState::Undefined, ResourceState::CopyDst)]);
    e.copy_texture_to_buffer(rt, SUB0, rb, 0, Format::Rgba8Unorm.copy_row_pitch(W));
}

/// Waits for the GPU and returns the W x H RGBA8 pixels of a readback buffer.
pub fn pixels(dev: &dyn Device, rb: BufferId) -> Vec<[u8; 4]> {
    dev.wait_idle();
    let mut out = Vec::new();
    dev.read_buffer(rb, &mut out).expect("read_buffer");
    let pitch = Format::Rgba8Unorm.copy_row_pitch(W) as usize;
    let mut px = Vec::with_capacity((W * H) as usize);
    for y in 0..H as usize {
        for x in 0..W as usize {
            let o = y * pitch + x * 4;
            px.push([out[o], out[o + 1], out[o + 2], out[o + 3]]);
        }
    }
    px
}

pub fn at(px: &[[u8; 4]], x: u32, y: u32) -> [u8; 4] {
    px[(y * W + x) as usize]
}

pub fn constants(words: &[u32]) -> [u8; DRAW_CONSTANTS_BYTES] {
    let mut c = [0u8; DRAW_CONSTANTS_BYTES];
    for (i, w) in words.iter().enumerate() {
        c[i * 4..i * 4 + 4].copy_from_slice(&w.to_ne_bytes());
    }
    c
}

pub fn constants_f(words: &[f32]) -> [u8; DRAW_CONSTANTS_BYTES] {
    constants(&words.iter().map(|f| f.to_bits()).collect::<Vec<_>>())
}

pub fn pipeline(dev: &dyn Device, vs: ShaderId, fs: ShaderId, depth: Option<DepthDesc>) -> RenderPipelineId {
    dev.create_render_pipeline(&RenderPipelineDesc {
        label: "test pipeline",
        vertex: vs,
        fragment: Some(fs),
        color_formats: &[Format::Rgba8Unorm],
        blend: Blend::Opaque,
        depth,
        cull: Cull::Back,
        topology: Topology::Triangles,
        samples: 1,
    })
    .expect("render pipeline")
}

pub fn compute_pipeline(dev: &dyn Device, cs: ShaderId, group: u32) -> ComputePipelineId {
    dev.create_compute_pipeline(&ComputePipelineDesc { label: "test kernel", shader: cs, workgroup_size: [group, 1, 1] }).expect("compute pipeline")
}

/// MSL fragment shader that outputs the flat colour from the vertex stage.
pub const FLAT_FS: &str = r#"
struct VOut { float4 pos [[position]]; float4 col [[flat]]; float2 uv; };
struct Empty { uint pad[16]; };
fragment float4 fs_flat(VOut in [[stage_in]], ED_BINDLESS(Empty)) { return in.col; }
"#;

/// Times a heavy compute pass and a one-group pass; returns (heavy ms, light ms).
pub fn heavy_and_light(dev: &dyn Device) -> (f32, f32) {
    const N: u32 = 1 << 20;
    let cs = shader(
        dev,
        ShaderStage::Compute,
        "burn",
        r#"
struct KC { uint buf; uint iters; uint pad[14]; };
kernel void burn(uint gid [[thread_position_in_grid]], ED_BINDLESS(KC)) {
    float x = float(gid);
    for (uint i = 0; i < ed_c.iters; i++) { x = fma(x, 1.000001, 0.5); x = sin(x); }
    ed_buf(float, ed_c.buf)[gid] = x;
}
"#,
    );
    let pso = compute_pipeline(dev, cs, 256);
    let out = dev.create_buffer(&BufferDesc { label: "burn out", size: N as u64 * 4, usage: Usage { shader_write: true, ..usage() } }).unwrap();
    let bi = dev.bind_buffer(out).unwrap();
    let mut t = Vec::new();
    let mut result = (0.0, 0.0);
    // Two rounds: the first warms up pipelines and clock calibration.
    for _ in 0..2 {
        let mut e = dev.begin_frame().unwrap();
        e.transition(&[buf(out, ResourceState::Undefined, ResourceState::ShaderWrite)]);
        e.begin_timer("heavy");
        e.begin_compute_pass("heavy");
        e.set_compute_pipeline(pso);
        e.set_compute_constants(&constants(&[bi.0, 2000]));
        e.dispatch(N / 256, 1, 1);
        e.end_compute_pass();
        e.end_timer();
        e.transition(&[buf(out, ResourceState::ShaderWrite, ResourceState::ShaderWrite)]);
        e.begin_timer("light");
        e.begin_compute_pass("light");
        e.set_compute_pipeline(pso);
        e.set_compute_constants(&constants(&[bi.0, 1]));
        e.dispatch(1, 1, 1);
        e.end_compute_pass();
        e.end_timer();
        dev.end_frame(e).unwrap();
        dev.wait_idle();
        dev.timings(&mut t);
        let get = |l: &str| t.iter().find(|x| x.label == l).map(|x| x.gpu_ms).unwrap_or(0.0);
        result = (get("heavy"), get("light"));
    }
    result
}

