//! The renderer: instance pools, GPU culling, geometry providers, lighting, atmosphere, post, picking. Talks to the GPU only through `ed_gpu`.
//!
//! Allowed workspace dependencies: `ed_geo`, `ed_gpu`, `ed_pack`, `ed_planet`. The list is enforced by
//! `cargo xtask check-deps` (rules in `xtask/deps.toml`).
//!
//! Status: v0 (Phase 1, M1b). [`graph`] is the frame graph (P1-D3). [`Renderer`] draws one
//! [`View`] through it: `sky` (or `clear`) -> `scene` -> `tonemap` -> `overlay`, into an HDR
//! `Rgba16Float` target with reverse-Z `Depth32Float` depth (P1-D8), then into the view's
//! output target (a surface texture or an offscreen texture). The scene is a fixed test scene
//! (ground and a few boxes) whose instances are read from a bindless buffer.
//!
//! Shaders are MSL source (`shaders/*.metal`, the MSL-first fallback of P1-D1), each file
//! prefixed with `ed_gpu::shader_abi::MSL_PRELUDE` and `shaders/common.metal`.

pub mod graph;

use ed_gpu::{
    BindlessBuffer, Blend, BufferDesc, BufferId, CommandEncoder, Compare, Cull, DRAW_CONSTANTS_BYTES, DepthDesc, Device, Format, LoadOp,
    RenderPipelineDesc, RenderPipelineId, ResourceState, ShaderCode, ShaderDesc, ShaderId, ShaderStage, StoreOp, Subresource, Timing,
    Topology, Usage,
};
use glam::{DVec3, Mat4, Quat, Vec3};
use graph::{FrameGraph, FrameTimings, GraphError, PassKind, TextureInfo, TransientPool};

/// HDR scene colour format.
pub const HDR_FORMAT: Format = Format::Rgba16Float;
/// Scene depth format (reverse-Z, P1-D8).
pub const DEPTH_FORMAT: Format = Format::Depth32Float;

const PRELUDE_COMMON: &str = include_str!("../shaders/common.metal");
const SKY_SRC: &str = include_str!("../shaders/sky.metal");
const SCENE_SRC: &str = include_str!("../shaders/scene.metal");
const TONEMAP_SRC: &str = include_str!("../shaders/tonemap.metal");

/// Pass names of a full frame, in run order (also the timer labels).
pub const PASSES_FULL: [&str; 4] = ["sky", "scene", "tonemap", "overlay"];
/// Pass names of an empty frame (`clear` + `tonemap` + `overlay`).
pub const PASSES_EMPTY: [&str; 3] = ["clear", "tonemap", "overlay"];

// ---------------------------------------------------------------------------------------------
// Camera and projection
// ---------------------------------------------------------------------------------------------

/// The one projection helper (P1-D8): right-handed, looking down -Z, reverse-Z depth in 0..1
/// with an infinite far plane (near maps to 1, infinity to 0). Use `GreaterEqual` depth tests
/// and clear depth to 0.
pub fn reverse_z_projection(fov_y: f32, aspect: f32, near: f32) -> Mat4 {
    // Written out (rather than a glam helper, whose names keep moving): z_ndc = near / -z_view.
    let f = 1.0 / (0.5 * fov_y).tan();
    Mat4::from_cols(
        glam::Vec4::new(f / aspect, 0.0, 0.0, 0.0),
        glam::Vec4::new(0.0, f, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, 0.0, -1.0),
        glam::Vec4::new(0.0, 0.0, near, 0.0),
    )
}

/// A camera: f64 position (truth), f32 orientation. Rendering is camera-relative, so the
/// position never reaches the GPU (P1-D7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub position: DVec3,
    /// Radians about +Y; 0 looks down -Z, positive turns left.
    pub yaw: f32,
    /// Radians; positive looks up.
    pub pitch: f32,
    /// Vertical field of view, radians.
    pub fov_y: f32,
    /// Near plane distance, metres.
    pub near: f32,
}

impl Camera {
    pub const DEFAULT_FOV_Y: f32 = 60.0 * std::f32::consts::PI / 180.0;
    pub const DEFAULT_NEAR: f32 = 0.1;

    /// A camera at `position` looking at `target`.
    pub fn look_at(position: DVec3, target: DVec3) -> Self {
        let d = (target - position).normalize().as_vec3();
        Self { position, yaw: (-d.x).atan2(-d.z), pitch: d.y.clamp(-1.0, 1.0).asin(), fov_y: Self::DEFAULT_FOV_Y, near: Self::DEFAULT_NEAR }
    }
    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(self.pitch)
    }
    pub fn forward(&self) -> Vec3 {
        self.rotation() * Vec3::NEG_Z
    }
    pub fn right(&self) -> Vec3 {
        self.rotation() * Vec3::X
    }
    /// Projection times the rotation-only view matrix: camera-relative positions to clip.
    pub fn relative_view_proj(&self, aspect: f32) -> Mat4 {
        reverse_z_projection(self.fov_y, aspect, self.near) * Mat4::from_quat(self.rotation().inverse())
    }
}

/// A pixel rectangle in the output target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Where a view's final image goes: a surface texture or an offscreen texture.
#[derive(Clone, Copy, Debug)]
pub struct ViewTarget {
    pub texture: ed_gpu::TextureId,
    pub format: Format,
    pub width: u32,
    pub height: u32,
    /// State the texture is in now (`Undefined` for a fresh surface texture).
    pub initial: ResourceState,
    /// State to leave it in (`Present` for a surface texture).
    pub final_state: Option<ResourceState>,
}

/// One view: camera, viewport and target. Multi-view (cascades, editor viewports) builds
/// on this later (P1-D24).
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub camera: Camera,
    pub viewport: Viewport,
    pub target: ViewTarget,
}

/// What to draw this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneMode {
    /// Sky, test scene, tonemap, overlay.
    TestScene,
    /// Clear, tonemap, overlay: the "empty frame" budget.
    Empty,
}

#[derive(Clone, Copy, Debug)]
pub struct FrameParams {
    pub mode: SceneMode,
    /// Unit vector towards the sun.
    pub sun_dir: Vec3,
    pub exposure: f32,
    /// Copy the output target into this `cpu_read` buffer (rows `Format::copy_row_pitch`
    /// apart) after the overlay pass. The output target needs `copy_src` usage.
    pub readback: Option<BufferId>,
}

impl Default for FrameParams {
    fn default() -> Self {
        Self { mode: SceneMode::TestScene, sun_dir: Vec3::new(0.45, 0.62, 0.35).normalize(), exposure: 1.0, readback: None }
    }
}

// ---------------------------------------------------------------------------------------------
// Test scene
// ---------------------------------------------------------------------------------------------

/// One box instance of the test scene (layout mirrors `Instance` in `scene.metal`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxInstance {
    /// Position relative to the scene's node origin.
    pub position: Vec3,
    pub yaw: f32,
    pub scale: Vec3,
    /// Linear albedo.
    pub color: Vec3,
    pub checker: bool,
}

/// The fixed test scene: node origin (far from 0 on purpose, to exercise camera-relative
/// rendering), a ground slab and five boxes.
pub struct TestScene;

impl TestScene {
    /// The node origin, f64. Instances are relative to it.
    pub const ORIGIN: DVec3 = DVec3::new(6_378_137.0, 0.0, -1_250_000.0);
    /// Centre of the red box, relative to the origin (the default look-at point).
    pub const FOCUS: Vec3 = Vec3::new(0.0, 1.0, 0.0);

    pub fn instances() -> Vec<BoxInstance> {
        let b = |p: [f32; 3], yaw: f32, s: [f32; 3], c: [f32; 3]| BoxInstance {
            position: Vec3::from(p),
            yaw,
            scale: Vec3::from(s),
            color: Vec3::from(c),
            checker: false,
        };
        vec![
            BoxInstance { checker: true, ..b([0.0, -0.25, 0.0], 0.0, [400.0, 0.5, 400.0], [0.32, 0.36, 0.30]) }, // ground
            b([0.0, 1.0, 0.0], 0.0, [2.0, 2.0, 2.0], [0.75, 0.08, 0.06]),                                        // red, the focus
            b([6.0, 0.75, -4.0], 0.4, [1.5, 1.5, 1.5], [0.10, 0.55, 0.12]),                                      // green
            b([-6.0, 1.5, -3.0], 0.6, [3.0, 3.0, 3.0], [0.08, 0.18, 0.70]),                                      // blue
            b([4.0, 0.5, 5.0], 0.2, [1.0, 1.0, 1.0], [0.80, 0.70, 0.10]),                                        // yellow
            b([-8.0, 4.0, -12.0], 0.0, [1.5, 8.0, 1.5], [0.80, 0.80, 0.78]),                                     // white tower
        ]
    }

    /// The camera the image tests use: south of the focus, looking at it.
    pub fn test_camera() -> Camera {
        let focus = Self::ORIGIN + Self::FOCUS.as_dvec3();
        Camera::look_at(focus + DVec3::new(0.0, 5.0, 16.0), focus)
    }
}

/// Unit cube, 24 vertices (position, normal; each padded to 16 bytes), 36 indices, CCW
/// seen from outside.
fn cube_mesh() -> (Vec<f32>, Vec<u32>) {
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ];
    let mut verts = Vec::with_capacity(24 * 8);
    let mut idx = Vec::with_capacity(36);
    for (fi, (n, u, v)) in faces.iter().enumerate() {
        let (n, u, v) = (Vec3::from(*n), Vec3::from(*u), Vec3::from(*v));
        debug_assert!(u.cross(v).abs_diff_eq(n, 1e-6));
        for (su, sv) in [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
            let p = n * 0.5 + u * su + v * sv;
            verts.extend_from_slice(&[p.x, p.y, p.z, 1.0, n.x, n.y, n.z, 0.0]);
        }
        let b = fi as u32 * 4;
        idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    (verts, idx)
}

fn f32_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

fn u32_bytes(v: &[u32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

/// Packs 32-bit words into a draw-constant block.
fn constants(words: &[u32]) -> [u8; DRAW_CONSTANTS_BYTES] {
    let mut c = [0u8; DRAW_CONSTANTS_BYTES];
    for (i, w) in words.iter().enumerate() {
        c[i * 4..i * 4 + 4].copy_from_slice(&w.to_ne_bytes());
    }
    c
}

/// Size of `FrameData` in `common.metal`, in floats.
const FRAME_DATA_FLOATS: usize = 16 * 2 + 4 * 7;

// ---------------------------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------------------------

/// A persistent buffer and the state the last frame left it in.
struct Tracked {
    id: BufferId,
    bindless: BindlessBuffer,
    state: ResourceState,
}

/// Renderer v0. Create once per device; call [`Renderer::render`] between `begin_frame` and
/// `end_frame`; [`Renderer::destroy`] before dropping the device.
pub struct Renderer {
    shaders: Vec<ShaderId>,
    sky: RenderPipelineId,
    scene: RenderPipelineId,
    tonemap_vs: ShaderId,
    tonemap_fs: ShaderId,
    tonemap: Vec<(Format, RenderPipelineId)>,
    vertices: Tracked,
    indices: Tracked,
    instances: Tracked,
    frame_data: Tracked,
    instance_count: u32,
    index_count: u32,
    pool: TransientPool,
    scratch: Vec<Timing>,
}

fn shader(dev: &dyn Device, label: &str, stage: ShaderStage, entry: &str, body: &str) -> Result<ShaderId, ed_gpu::Error> {
    let src = format!("{}\n{}\n{}", ed_gpu::shader_abi::MSL_PRELUDE, PRELUDE_COMMON, body);
    dev.create_shader(&ShaderDesc { label, stage, entry, code: ShaderCode::MslSource(&src) })
}

fn static_buffer(dev: &dyn Device, label: &str, data: &[u8]) -> Result<Tracked, ed_gpu::Error> {
    let usage = Usage { shader_read: true, copy_dst: true, ..Default::default() };
    let id = dev.create_buffer(&BufferDesc { label, size: data.len() as u64, usage })?;
    dev.write_buffer(id, 0, data)?;
    let bindless = dev.bind_buffer(id)?;
    // The staged upload leaves the buffer in CopyDst; the first frame's graph moves it on.
    Ok(Tracked { id, bindless, state: ResourceState::CopyDst })
}

impl Renderer {
    pub fn new(dev: &dyn Device) -> Result<Self, ed_gpu::Error> {
        let sky_vs = shader(dev, "sky vs", ShaderStage::Vertex, "vs_fullscreen", SKY_SRC)?;
        let sky_fs = shader(dev, "sky fs", ShaderStage::Fragment, "fs_sky", SKY_SRC)?;
        let scene_vs = shader(dev, "scene vs", ShaderStage::Vertex, "vs_scene", SCENE_SRC)?;
        let scene_fs = shader(dev, "scene fs", ShaderStage::Fragment, "fs_scene", SCENE_SRC)?;
        let tonemap_vs = shader(dev, "tonemap vs", ShaderStage::Vertex, "vs_fullscreen", TONEMAP_SRC)?;
        let tonemap_fs = shader(dev, "tonemap fs", ShaderStage::Fragment, "fs_tonemap", TONEMAP_SRC)?;
        let sky = dev.create_render_pipeline(&RenderPipelineDesc {
            label: "sky",
            vertex: sky_vs,
            fragment: Some(sky_fs),
            color_formats: &[HDR_FORMAT],
            blend: Blend::Opaque,
            depth: None,
            cull: Cull::None,
            topology: Topology::Triangles,
            samples: 1,
        })?;
        let scene = dev.create_render_pipeline(&RenderPipelineDesc {
            label: "scene",
            vertex: scene_vs,
            fragment: Some(scene_fs),
            color_formats: &[HDR_FORMAT],
            blend: Blend::Opaque,
            depth: Some(DepthDesc { format: DEPTH_FORMAT, write: true, compare: Compare::GreaterEqual, bias: 0.0, slope_bias: 0.0 }),
            cull: Cull::Back,
            topology: Topology::Triangles,
            samples: 1,
        })?;
        let (verts, idx) = cube_mesh();
        let vertices = static_buffer(dev, "cube vertices", &f32_bytes(&verts))?;
        let indices = static_buffer(dev, "cube indices", &u32_bytes(&idx))?;
        let inst = TestScene::instances();
        let mut words = Vec::with_capacity(inst.len() * 12);
        for i in &inst {
            words.extend_from_slice(&[i.position.x, i.position.y, i.position.z, i.yaw, i.scale.x, i.scale.y, i.scale.z, 0.0]);
            words.extend_from_slice(&[i.color.x, i.color.y, i.color.z, if i.checker { 1.0 } else { 0.0 }]);
        }
        let instances = static_buffer(dev, "test scene instances", &f32_bytes(&words))?;
        let fd = dev.create_buffer(&BufferDesc {
            label: "frame data",
            size: (FRAME_DATA_FLOATS * 4) as u64,
            usage: Usage { shader_read: true, cpu_write: true, ..Default::default() },
        })?;
        let frame_data = Tracked { id: fd, bindless: dev.bind_buffer(fd)?, state: ResourceState::Undefined };
        Ok(Self {
            shaders: vec![sky_vs, sky_fs, scene_vs, scene_fs],
            sky,
            scene,
            tonemap_vs,
            tonemap_fs,
            tonemap: Vec::new(),
            vertices,
            indices,
            instances,
            frame_data,
            instance_count: inst.len() as u32,
            index_count: idx.len() as u32,
            pool: TransientPool::new(),
            scratch: Vec::new(),
        })
    }

    /// Pass names (timer labels) of a frame in `mode`, in run order.
    pub fn pass_names(mode: SceneMode) -> &'static [&'static str] {
        match mode {
            SceneMode::TestScene => &PASSES_FULL,
            SceneMode::Empty => &PASSES_EMPTY,
        }
    }

    fn tonemap_pipeline(&mut self, dev: &dyn Device, format: Format) -> Result<RenderPipelineId, ed_gpu::Error> {
        if let Some((_, p)) = self.tonemap.iter().find(|(f, _)| *f == format) {
            return Ok(*p);
        }
        let p = dev.create_render_pipeline(&RenderPipelineDesc {
            label: "tonemap",
            vertex: self.tonemap_vs,
            fragment: Some(self.tonemap_fs),
            color_formats: &[format],
            blend: Blend::Opaque,
            depth: None,
            cull: Cull::None,
            topology: Topology::Triangles,
            samples: 1,
        })?;
        self.tonemap.push((format, p));
        Ok(p)
    }

    /// Record one view into `enc`. Call between `begin_frame` and `end_frame`.
    pub fn render(&mut self, dev: &dyn Device, enc: &mut dyn CommandEncoder, view: &View, params: &FrameParams) -> Result<(), GraphError> {
        let vp = view.viewport;
        let (w, h) = (vp.width.max(1), vp.height.max(1));
        let tonemap = self.tonemap_pipeline(dev, view.target.format)?;

        // Per-view data, camera-relative (P1-D7).
        let cam = &view.camera;
        let vpm = cam.relative_view_proj(w as f32 / h as f32);
        let mut fd = [0f32; FRAME_DATA_FLOATS];
        fd[0..16].copy_from_slice(&vpm.to_cols_array());
        fd[16..32].copy_from_slice(&vpm.inverse().to_cols_array());
        let s = params.sun_dir.normalize_or(Vec3::Y);
        let rows: [[f32; 4]; 7] = [
            [s.x, s.y, s.z, 0.0],
            [3.0, 2.85, 2.6, 0.0],    // sun
            [0.18, 0.36, 0.85, 0.0],  // zenith
            [0.62, 0.72, 0.86, 0.0],  // horizon
            [0.24, 0.21, 0.17, 0.0],  // ground
            [vp.x as f32, vp.y as f32, w as f32, h as f32],
            [params.exposure, 0.0, 0.0, 0.0],
        ];
        for (i, r) in rows.iter().enumerate() {
            fd[32 + i * 4..36 + i * 4].copy_from_slice(r);
        }
        let fd_bytes = f32_bytes(&fd);
        dev.fill_frame_buffer(self.frame_data.id, &mut |dst| dst[..fd_bytes.len()].copy_from_slice(&fd_bytes))?;
        // The only use of the f64 camera position: one camera-relative origin per view.
        let origin_rel = (TestScene::ORIGIN - cam.position).as_vec3();

        let fdi = self.frame_data.bindless.0;
        let (sky, scene) = (self.sky, self.scene);
        let (verts, inst, idx_buf) = (self.vertices.bindless.0, self.instances.bindless.0, self.indices.id);
        let (instance_count, index_count) = (self.instance_count, self.index_count);
        let encode_srgb = !matches!(view.target.format, Format::Rgba8Srgb | Format::Bgra8Srgb | Format::Rgba16Float);
        let exposure = params.exposure;

        let mut g = FrameGraph::new();
        let hdr = g.create_texture("hdr", TextureInfo { format: HDR_FORMAT, width: w, height: h });
        let out_info = TextureInfo { format: view.target.format, width: view.target.width, height: view.target.height };
        let out = g.import_texture("output", view.target.texture, out_info, view.target.initial, view.target.final_state);
        let fd_res = g.import_buffer("frame data", self.frame_data.id, self.frame_data.state, None);
        let mut statics = None;
        match params.mode {
            SceneMode::TestScene => {
                let depth = g.create_texture("depth", TextureInfo { format: DEPTH_FORMAT, width: w, height: h });
                let v = g.import_buffer("vertices", self.vertices.id, self.vertices.state, None);
                let i = g.import_buffer("indices", self.indices.id, self.indices.state, None);
                let n = g.import_buffer("instances", self.instances.id, self.instances.state, None);
                statics = Some((v, i, n));
                g.add_pass("sky", PassKind::Raster).read(fd_res, ResourceState::ShaderRead).color(hdr, LoadOp::Discard, StoreOp::Store).exec(
                    move |ctx| {
                        ctx.enc.set_render_pipeline(sky);
                        ctx.enc.set_draw_constants(&constants(&[fdi]));
                        ctx.enc.draw(0..3, 0..1);
                    },
                );
                g.add_pass("scene", PassKind::Raster)
                    .read(fd_res, ResourceState::ShaderRead)
                    .read(v, ResourceState::ShaderRead)
                    .read(i, ResourceState::ShaderRead)
                    .read(n, ResourceState::ShaderRead)
                    .color(hdr, LoadOp::Load, StoreOp::Store)
                    .depth(depth, LoadOp::Clear([0.0; 4]), StoreOp::Discard, true)
                    .exec(move |ctx| {
                        ctx.enc.set_render_pipeline(scene);
                        ctx.enc.set_index_buffer(idx_buf, 0);
                        let o = origin_rel;
                        ctx.enc.set_draw_constants(&constants(&[fdi, inst, verts, 0, o.x.to_bits(), o.y.to_bits(), o.z.to_bits(), 0]));
                        ctx.enc.draw_indexed(0..index_count, 0, 0..instance_count);
                    });
            }
            SceneMode::Empty => {
                g.add_pass("clear", PassKind::Raster).color(hdr, LoadOp::Clear([0.62, 0.72, 0.86, 1.0]), StoreOp::Store).done();
            }
        }
        let full = vp.x == 0 && vp.y == 0 && vp.width == view.target.width && vp.height == view.target.height;
        let out_load = if full { LoadOp::Discard } else { LoadOp::Clear([0.0, 0.0, 0.0, 1.0]) };
        g.add_pass("tonemap", PassKind::Raster).read(hdr, ResourceState::ShaderRead).color(out, out_load, StoreOp::Store).exec(move |ctx| {
            if !full {
                ctx.enc.set_viewport(vp.x as f32, vp.y as f32, w as f32, h as f32);
            }
            let hdr_idx = ctx.bindless(hdr).0;
            ctx.enc.set_render_pipeline(tonemap);
            ctx.enc.set_draw_constants(&constants(&[
                hdr_idx,
                encode_srgb as u32,
                exposure.to_bits(),
                0,
                (vp.x as f32).to_bits(),
                (vp.y as f32).to_bits(),
                (w as f32).to_bits(),
                (h as f32).to_bits(),
            ]));
            ctx.enc.draw(0..3, 0..1);
        });
        // Reserved for the egui painter (P1-D14). Declares the output so it orders after
        // tonemap; records nothing yet.
        g.add_pass("overlay", PassKind::Manual).write(out, ResourceState::RenderTarget).done();
        if let Some(rb) = params.readback {
            let rb_res = g.import_buffer("readback", rb, ResourceState::Undefined, None);
            let pitch = view.target.format.copy_row_pitch(view.target.width);
            g.add_pass("readback", PassKind::Manual).read(out, ResourceState::CopySrc).write(rb_res, ResourceState::CopyDst).exec(move |ctx| {
                let (t, b) = (ctx.texture(out), ctx.buffer(rb_res));
                ctx.enc.copy_texture_to_buffer(t, Subresource { mip: 0, layer: 0 }, b, 0, pitch);
            });
        }
        let plan = g.execute(dev, enc, &mut self.pool)?;
        self.frame_data.state = plan.final_states[fd_res.index()];
        if let Some((v, i, n)) = statics {
            self.vertices.state = plan.final_states[v.index()];
            self.indices.state = plan.final_states[i.index()];
            self.instances.state = plan.final_states[n.index()];
        }
        Ok(())
    }

    /// Per-pass GPU timings of the most recent finished frame (results lag a few frames).
    pub fn timings(&mut self, dev: &dyn Device) -> Option<FrameTimings> {
        FrameTimings::read(dev, &mut self.scratch)
    }

    /// Transient textures created so far (stops growing after warm-up at a fixed size).
    pub fn transient_textures_created(&self) -> u64 {
        self.pool.textures_created()
    }

    /// Release every GPU object. Call after `wait_idle`, before dropping the device.
    pub fn destroy(mut self, dev: &dyn Device) {
        self.pool.destroy(dev);
        for t in [&self.vertices, &self.indices, &self.instances, &self.frame_data] {
            dev.unbind_buffer(t.bindless);
            dev.destroy_buffer(t.id);
        }
        dev.destroy_render_pipeline(self.sky);
        dev.destroy_render_pipeline(self.scene);
        for (_, p) in self.tonemap.drain(..) {
            dev.destroy_render_pipeline(p);
        }
        for s in self.shaders.drain(..).chain([self.tonemap_vs, self.tonemap_fs]) {
            dev.destroy_shader(s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_is_reverse_z_infinite() {
        let p = reverse_z_projection(1.0, 1.0, 0.1);
        let near = p * glam::Vec4::new(0.0, 0.0, -0.1, 1.0);
        let far = p * glam::Vec4::new(0.0, 0.0, -1.0e9, 1.0);
        assert!((near.z / near.w - 1.0).abs() < 1e-6, "near plane maps to 1");
        assert!((far.z / far.w).abs() < 1e-6, "infinity maps to 0");
    }

    #[test]
    fn look_at_points_forward() {
        let c = Camera::look_at(DVec3::new(1e7, 5.0, 16.0), DVec3::new(1e7, 1.0, 0.0));
        let want = Vec3::new(0.0, -4.0, -16.0).normalize();
        assert!(c.forward().abs_diff_eq(want, 1e-5), "{:?}", c.forward());
        // The target projects to the centre of the screen.
        let rel = (DVec3::new(1e7, 1.0, 0.0) - c.position).as_vec3();
        let clip = c.relative_view_proj(1.5) * rel.extend(1.0);
        assert!((clip.x / clip.w).abs() < 1e-5 && (clip.y / clip.w).abs() < 1e-5);
    }

    #[test]
    fn cube_is_ccw_from_outside() {
        let (v, idx) = cube_mesh();
        let p = |i: u32| Vec3::new(v[i as usize * 8], v[i as usize * 8 + 1], v[i as usize * 8 + 2]);
        for t in idx.chunks(3) {
            let n = Vec3::new(v[t[0] as usize * 8 + 4], v[t[0] as usize * 8 + 5], v[t[0] as usize * 8 + 6]);
            let face = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
            assert!(face.dot(n) > 0.0);
        }
    }
}
