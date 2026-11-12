//! Shared helpers for the backend tests. Each test body takes a `&dyn Device`;
//! [`backend_tests!`] runs it once per compiled-in backend (Metal on macOS, Vulkan with the
//! `vulkan` feature). Shaders come in pairs: MSL source (compiled by the Metal backend) and
//! SPIR-V built from `tests/shaders/*.glsl` (see `tests/shaders/build.sh`).
#![allow(dead_code)]

use ed_gpu::*;

pub const W: u32 = 64;
pub const H: u32 = 64;

/// The device for `backend` with validation on. Metal must exist on macOS (panics if not).
/// Vulkan may be missing (no loader or no MoltenVK): the test then prints why and is skipped,
/// unless `ED_GPU_REQUIRE_VULKAN=1`, which turns a missing Vulkan into a failure.
pub fn device(backend: Backend) -> Option<Box<dyn Device>> {
    match create_device(
        backend,
        &DeviceDesc {
            validation: true,
            frames_in_flight: 2,
        },
    ) {
        Ok(d) => Some(d),
        Err(e)
            if backend == Backend::Vulkan
                && std::env::var("ED_GPU_REQUIRE_VULKAN").as_deref() != Ok("1") =>
        {
            eprintln!("SKIP: no Vulkan device ({e})");
            None
        }
        Err(e) => panic!("{backend:?} device: {e}"),
    }
}

/// Generates `mod metal` and `mod vulkan`, each with one `#[test]` per listed body
/// `fn name(dev: &dyn Device)` from the enclosing file. With a leading `exclusive;` every test
/// holds [`exclusive`] from before its device is created until it is dropped.
#[macro_export]
macro_rules! backend_tests {
    (exclusive; $($name:ident),* $(,)?) => { $crate::backend_tests!(@gen ($crate::common::exclusive()) $($name),*); };
    ($($name:ident),* $(,)?) => { $crate::backend_tests!(@gen (()) $($name),*); };
    (@gen ($guard:expr) $($name:ident),*) => {
        #[cfg(target_os = "macos")]
        mod metal {
            $( #[test] fn $name() { let _gpu = $guard; if let Some(d) = $crate::common::device(ed_gpu::Backend::Metal) { super::$name(&*d) } } )*
        }
        #[cfg(feature = "vulkan")]
        mod vulkan {
            $( #[test] fn $name() { let _gpu = $guard; if let Some(d) = $crate::common::device(ed_gpu::Backend::Vulkan) { super::$name(&*d) } } )*
        }
    };
}

/// The checked-in SPIR-V for `tests/shaders/<name>` (e.g. `spv!("tri.vert")`).
#[macro_export]
macro_rules! spv {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/shaders/spv/",
            $name,
            ".spv"
        ))
        .as_slice()
    };
}

pub fn usage() -> Usage {
    Usage::default()
}

/// A shader for whichever backend `dev` is: `msl` (the body after [`shader_abi::MSL_PRELUDE`])
/// with entry point `entry` on Metal, `spv` with entry `main` on Vulkan.
pub fn shader(
    dev: &dyn Device,
    stage: ShaderStage,
    entry: &str,
    msl: &str,
    spv: &[u8],
) -> ShaderId {
    let r = match dev.caps().backend {
        Backend::Vulkan => {
            let words = spirv_words(spv);
            dev.create_shader(&ShaderDesc {
                label: entry,
                stage,
                entry: "main",
                code: ShaderCode::SpirV(&words),
            })
        }
        _ => {
            let src = format!("{}\n{}", shader_abi::MSL_PRELUDE, msl);
            dev.create_shader(&ShaderDesc {
                label: entry,
                stage,
                entry,
                code: ShaderCode::MslSource(&src),
            })
        }
    };
    r.unwrap_or_else(|e| panic!("shader {entry}: {e}"))
}

pub fn spirv_words(bytes: &[u8]) -> Vec<u32> {
    assert_eq!(bytes.len() % 4, 0, "SPIR-V length must be a multiple of 4");
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
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
        usage: Usage {
            render_target: true,
            copy_src: true,
            ..usage()
        },
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
        usage: Usage {
            depth: true,
            ..usage()
        },
    })
    .expect("depth target")
}

pub fn readback_buffer(dev: &dyn Device, size: u64) -> BufferId {
    dev.create_buffer(&BufferDesc {
        label: "readback",
        size,
        usage: Usage {
            copy_dst: true,
            cpu_read: true,
            ..usage()
        },
    })
    .expect("readback buffer")
}

pub fn rt_readback(dev: &dyn Device) -> BufferId {
    readback_buffer(dev, (Format::Rgba8Unorm.copy_row_pitch(W) * H) as u64)
}

pub const SUB0: Subresource = Subresource { mip: 0, layer: 0 };

pub fn tex(texture: TextureId, from: ResourceState, to: ResourceState) -> Transition {
    Transition::Texture {
        texture,
        sub: None,
        from,
        to,
    }
}

pub fn buf(buffer: BufferId, from: ResourceState, to: ResourceState) -> Transition {
    Transition::Buffer { buffer, from, to }
}

pub fn color(texture: TextureId, load: LoadOp) -> Attachment {
    Attachment {
        texture,
        sub: SUB0,
        load,
        store: StoreOp::Store,
    }
}

/// Records a copy of the RGBA8 render target into `rb` (state RenderTarget -> CopySrc).
pub fn copy_out(e: &mut dyn CommandEncoder, rt: TextureId, rb: BufferId) {
    e.transition(&[
        tex(rt, ResourceState::RenderTarget, ResourceState::CopySrc),
        buf(rb, ResourceState::Undefined, ResourceState::CopyDst),
    ]);
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

/// Waits for the GPU and returns a readback buffer as `u32`s.
pub fn words(dev: &dyn Device, rb: BufferId) -> Vec<u32> {
    dev.wait_idle();
    let mut out = Vec::new();
    dev.read_buffer(rb, &mut out).expect("read_buffer");
    out.as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_ne_bytes(*c))
        .collect()
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

pub fn bytes_u32(v: &[u32]) -> Vec<u8> {
    v.iter().flat_map(|w| w.to_ne_bytes()).collect()
}

pub fn pipeline(
    dev: &dyn Device,
    vs: ShaderId,
    fs: ShaderId,
    depth: Option<DepthDesc>,
) -> RenderPipelineId {
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
    dev.create_compute_pipeline(&ComputePipelineDesc {
        label: "test kernel",
        shader: cs,
        workgroup_size: [group, 1, 1],
    })
    .expect("compute pipeline")
}

/// MSL fragment shader that outputs the flat colour from the vertex stage (`flat.frag`).
pub const FLAT_FS: &str = r#"
struct VOut { float4 pos [[position]]; float4 col [[flat]]; float2 uv; };
struct Empty { uint pad[16]; };
fragment float4 fs_flat(VOut in [[stage_in]], ED_BINDLESS(Empty)) { return in.col; }
"#;

pub fn flat_fs(dev: &dyn Device) -> ShaderId {
    shader(
        dev,
        ShaderStage::Fragment,
        "fs_flat",
        FLAT_FS,
        spv!("flat.frag"),
    )
}

/// Full-screen triangle with uv in 0..1 (v down), MSL side of `full.vert`. Fragment code is
/// appended per test.
pub const FULLSCREEN_VS: &str = r#"
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

/// MSL side of `fill.comp`: `buf[i] = i * 3 + add` for i < n.
pub const FILL_MSL: &str = r#"
struct KC { uint buf; uint n; uint add; uint pad[13]; };
kernel void fill(uint gid [[thread_position_in_grid]], ED_BINDLESS(KC)) {
    if (gid < ed_c.n) { ed_buf(uint, ed_c.buf)[gid] = gid * 3u + ed_c.add; }
}
"#;

/// MSL side of `burn.comp`.
pub const BURN_MSL: &str = r#"
struct KC { uint buf; uint iters; uint pad[14]; };
kernel void burn(uint gid [[thread_position_in_grid]], ED_BINDLESS(KC)) {
    float x = float(gid);
    for (uint i = 0; i < ed_c.iters; i++) { x = fma(x, 1.000001, 0.5); x = sin(x); }
    ed_buf(float, ed_c.buf)[gid] = x;
}
"#;

/// Times a heavy compute pass and a one-group pass; returns (heavy ms, light ms).
pub fn heavy_and_light(dev: &dyn Device) -> (f32, f32) {
    const N: u32 = 1 << 20;
    let cs = shader(
        dev,
        ShaderStage::Compute,
        "burn",
        BURN_MSL,
        spv!("burn.comp"),
    );
    let pso = compute_pipeline(dev, cs, 256);
    let out = dev
        .create_buffer(&BufferDesc {
            label: "burn out",
            size: N as u64 * 4,
            usage: Usage {
                shader_write: true,
                ..usage()
            },
        })
        .unwrap();
    let bi = dev.bind_buffer(out).unwrap();
    let mut t = Vec::new();
    let mut result = (0.0, 0.0);
    // Two rounds: the first warms up pipelines and clock calibration.
    for _ in 0..2 {
        let mut e = dev.begin_frame().unwrap();
        e.transition(&[buf(
            out,
            ResourceState::Undefined,
            ResourceState::ShaderWrite,
        )]);
        e.begin_timer("heavy");
        e.begin_compute_pass("heavy");
        e.set_compute_pipeline(pso);
        e.set_compute_constants(&constants(&[bi.0, 2000]));
        e.dispatch(N / 256, 1, 1);
        e.end_compute_pass();
        e.end_timer();
        e.transition(&[buf(
            out,
            ResourceState::ShaderWrite,
            ResourceState::ShaderWrite,
        )]);
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
        let get = |l: &str| {
            t.iter()
                .find(|x| x.label == l)
                .map(|x| x.gpu_ms)
                .unwrap_or(0.0)
        };
        result = (get("heavy"), get("light"));
    }
    result
}

/// Serialises tests in one binary that must not share the GPU or the process-wide Metal
/// allocation counter with another test (MoltenVK allocates on the same `MTLDevice`).
pub fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
