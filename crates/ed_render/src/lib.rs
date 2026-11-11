//! The renderer: instance pools, GPU culling, geometry providers, lighting, atmosphere, post, picking. Talks to the GPU only through `ed_gpu`.
//!
//! Allowed workspace dependencies: `ed_geo`, `ed_gpu`, `ed_pack`, `ed_planet`. The list is enforced by
//! `cargo xtask check-deps` (rules in `xtask/deps.toml`).
//!
//! Status: M2 (Phase 1, instances at scale). [`graph`] is the frame graph (P1-D3). [`Renderer`]
//! draws one [`View`] through it:
//!
//! `cull_early` -> `raster_early` -> `hzb` -> `cull_late` -> `raster_late` (provider 1,
//! [`provider::DiscreteLod`]) -> `resolve` -> `tonemap` -> `overlay`
//!
//! The rasters write reverse-Z `Depth32Float` depth (P1-D8) and an `Rg32Uint` visibility
//! buffer (instance + 1, LOD << 28 | primitive). `resolve` shades each pixel once from it
//! (sky where nothing was drawn) into an `Rgba16Float` HDR target, reading materials from a
//! bindless buffer indexed by material id. Optional `pop_stats` and `pick` passes read the
//! visibility buffer for the pop detector and for picking.
//!
//! Inputs ([`scene`]): meshes with LOD chains in one shared vertex and index buffer,
//! materials, and [`scene::InstancePools`] (f32 transforms relative to f64 node origins, stable
//! ids, dirty-range uploads). Call [`Renderer::flush_uploads`] before `begin_frame`; the data
//! lands at the start of that frame.
//!
//! Shaders are MSL source (`shaders/*.metal`, the MSL-first fallback of P1-D1), each file
//! prefixed with `ed_gpu::shader_abi::MSL_PRELUDE` and `shaders/common.metal`.

pub mod graph;
mod gpu;
pub mod provider;
pub mod scene;

use ed_geo::ObjectId;
use ed_gpu::{
    BindlessBuffer, Blend, BufferDesc, BufferId, CommandEncoder, ComputePipelineDesc, ComputePipelineId, Cull, Device, Format, LoadOp,
    RenderPipelineDesc, RenderPipelineId, ResourceState, ShaderCode, ShaderDesc, ShaderId, ShaderStage, StoreOp, Subresource, Timing,
    Topology, Usage,
};
use glam::{Affine3A, DVec3, Mat4, Quat, Vec3, Vec4};
use gpu::{FD_PARAMS, FD_PLANES, FD_PROJ, FD_SUN_DIR, FD_VIEWPORT, FRAME_WORDS, INSTANCE_WORDS, MATERIAL_WORDS, VERTEX_WORDS, constants};
use graph::{FrameGraph, FrameTimings, GraphError, PassKind, TextureInfo, TransientPool};
use provider::{CullStats, DiscreteLod, GeometryProvider, LodConfig, MeshRecord, Ring, SharedInputs, VisTargets};
use scene::{InstanceDesc, InstancePools, MAX_LODS, Material, MaterialId, MeshAsset, MeshId};

/// HDR scene colour format.
pub const HDR_FORMAT: Format = Format::Rgba16Float;
/// Scene depth format (reverse-Z, P1-D8).
pub const DEPTH_FORMAT: Format = Format::Depth32Float;
/// Visibility buffer: x = instance + 1 (0 = sky), y = LOD << 28 | primitive id.
pub const VIS_FORMAT: Format = Format::Rg32Uint;

const PRELUDE_COMMON: &str = include_str!("../shaders/common.metal");
const RESOLVE_SRC: &str = include_str!("../shaders/resolve.metal");
const TONEMAP_SRC: &str = include_str!("../shaders/tonemap.metal");
const DEBUG_SRC: &str = include_str!("../shaders/debug.metal");

/// Pass names of an empty frame (`clear` + `tonemap` + `overlay`).
pub const PASSES_EMPTY: [&str; 3] = ["clear", "tonemap", "overlay"];
/// Pop-detector tile size in pixels.
pub const POP_TILE: u32 = 8;

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
    /// Rotation-only view matrix (camera-relative world to view).
    pub fn relative_view(&self) -> Mat4 {
        Mat4::from_quat(self.rotation().inverse())
    }
    /// Projection times the rotation-only view matrix: camera-relative positions to clip.
    pub fn relative_view_proj(&self, aspect: f32) -> Mat4 {
        reverse_z_projection(self.fov_y, aspect, self.near) * self.relative_view()
    }
}

/// Frustum planes (left, right, bottom, top, near; inward normals, unit length) of a
/// camera-relative reverse-Z infinite projection. There is no far plane.
pub fn frustum_planes(view_proj: &Mat4) -> [Vec4; 5] {
    let r = |i| view_proj.row(i);
    let planes = [r(3) + r(0), r(3) - r(0), r(3) + r(1), r(3) - r(1), r(3) - r(2)];
    planes.map(|p| p / p.truncate().length())
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
    /// The scene (instances through the geometry provider), resolve, tonemap, overlay.
    Scene,
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
    /// Read the visibility buffer at this pixel (viewport coordinates); the answer arrives in
    /// [`Renderer::pick_result`] once the frame has finished.
    pub pick: Option<(u32, u32)>,
    /// Run the pop detector's per-tile pass ([`Renderer::take_pop_frames`]).
    pub pop_stats: bool,
}

impl Default for FrameParams {
    fn default() -> Self {
        Self { mode: SceneMode::Scene, sun_dir: Vec3::new(0.45, 0.62, 0.35).normalize(), exposure: 1.0, readback: None, pick: None, pop_stats: false }
    }
}

/// The answer to a [`FrameParams::pick`] request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickResult {
    pub frame: u64,
    pub x: u32,
    pub y: u32,
    /// GPU instance slot under the pixel (`None`: sky).
    pub instance: Option<u32>,
    /// The instance's object; `None` for sky or when the slot changed hands since.
    pub object: Option<ObjectId>,
    pub lod: u32,
    pub primitive: u32,
}

/// One frame of pop-detector tiles: per [`POP_TILE`]-square tile, the mean display luminance
/// (tonemapped, 0..1) and the number of pixels on an instance in a LOD transition.
#[derive(Clone, Debug, PartialEq)]
pub struct PopFrame {
    pub frame: u64,
    pub tiles_x: u32,
    pub tiles_y: u32,
    pub luma: Vec<f32>,
    pub transition_px: Vec<f32>,
}

// ---------------------------------------------------------------------------------------------
// Test scene
// ---------------------------------------------------------------------------------------------

/// One test-scene box: position, yaw, scale, colour, checker.
pub type TestBox = ([f32; 3], f32, [f32; 3], [f32; 3], bool);

/// The fixed test scene: node origin (far from 0 on purpose, to exercise camera-relative
/// rendering), a ground slab and five boxes, all instances of one cube mesh.
pub struct TestScene;

impl TestScene {
    /// The node origin, f64. Instances are relative to it.
    pub const ORIGIN: DVec3 = DVec3::new(6_378_137.0, 0.0, -1_250_000.0);
    /// Centre of the red box, relative to the origin (the default look-at point).
    pub const FOCUS: Vec3 = Vec3::new(0.0, 1.0, 0.0);

    /// (position, yaw, scale, colour, checker) of the six boxes.
    pub fn boxes() -> Vec<TestBox> {
        vec![
            ([0.0, -0.25, 0.0], 0.0, [400.0, 0.5, 400.0], [0.32, 0.36, 0.30], true), // ground
            ([0.0, 1.0, 0.0], 0.0, [2.0, 2.0, 2.0], [0.75, 0.08, 0.06], false),       // red, the focus
            ([6.0, 0.75, -4.0], 0.4, [1.5, 1.5, 1.5], [0.10, 0.55, 0.12], false),     // green
            ([-6.0, 1.5, -3.0], 0.6, [3.0, 3.0, 3.0], [0.08, 0.18, 0.70], false),     // blue
            ([4.0, 0.5, 5.0], 0.2, [1.0, 1.0, 1.0], [0.80, 0.70, 0.10], false),       // yellow
            ([-8.0, 4.0, -12.0], 0.0, [1.5, 8.0, 1.5], [0.80, 0.80, 0.78], false),    // white tower
        ]
    }

    /// Adds the scene to `r` (one node, one mesh, six materials, six instances).
    pub fn load(r: &mut Renderer, dev: &dyn Device) -> Result<Vec<scene::InstanceId>, ed_gpu::Error> {
        let mesh = r.add_mesh(dev, &cube_mesh())?;
        let node = r.instances().add_node(Self::ORIGIN).ok_or_else(|| ed_gpu::Error::Invalid("node table full".into()))?;
        let mut ids = Vec::new();
        for (i, (p, yaw, s, c, checker)) in Self::boxes().into_iter().enumerate() {
            let mut m = Material::pbr(Vec3::from(c), 0.0, 0.7);
            m.checker = checker;
            let mat = r.add_material(m).ok_or_else(|| ed_gpu::Error::Invalid("material table full".into()))?;
            let t = Affine3A::from_scale_rotation_translation(Vec3::from(s), Quat::from_rotation_y(yaw), Vec3::from(p));
            let id = r
                .instances()
                .add(&InstanceDesc { node, transform: t, mesh, material: mat, object: ObjectId(0x7E57_0000 + i as u128), flags: 0 })
                .ok_or_else(|| ed_gpu::Error::Invalid("instance pool full".into()))?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// The camera the image tests use: south of the focus, looking at it.
    pub fn test_camera() -> Camera {
        let focus = Self::ORIGIN + Self::FOCUS.as_dvec3();
        Camera::look_at(focus + DVec3::new(0.0, 5.0, 16.0), focus)
    }
}

/// Unit cube, 24 vertices, 36 indices, CCW seen from outside, one LOD.
pub fn cube_mesh() -> MeshAsset {
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ];
    let mut m = MeshAsset::default();
    let mut idx = Vec::with_capacity(36);
    for (fi, (n, u, v)) in faces.iter().enumerate() {
        let (n, u, v) = (Vec3::from(*n), Vec3::from(*u), Vec3::from(*v));
        debug_assert!(u.cross(v).abs_diff_eq(n, 1e-6));
        for (su, sv) in [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
            m.positions.push((n * 0.5 + u * su + v * sv).to_array());
            m.normals.push(n.to_array());
        }
        let b = fi as u32 * 4;
        idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    m.lods.push(scene::MeshLod { indices: idx, error: 0.0 });
    m
}

// ---------------------------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------------------------

/// A persistent buffer, its bindless index and the state the last frame left it in.
pub(crate) struct Tracked {
    pub id: BufferId,
    pub bindless: BindlessBuffer,
    pub state: ResourceState,
    pub size: u64,
}

impl Tracked {
    pub fn new(dev: &dyn Device, label: &str, size: u64, usage: Usage) -> Result<Self, ed_gpu::Error> {
        let id = dev.create_buffer(&BufferDesc { label, size: size.max(16), usage })?;
        Ok(Self { id, bindless: dev.bind_buffer(id)?, state: ResourceState::Undefined, size: size.max(16) })
    }
    pub fn destroy(self, dev: &dyn Device) {
        dev.unbind_buffer(self.bindless);
        dev.destroy_buffer(self.id);
    }
}

pub(crate) fn shader(dev: &dyn Device, label: &str, stage: ShaderStage, entry: &str, body: &str) -> Result<ShaderId, ed_gpu::Error> {
    let src = format!("{}\n{}\n{}", ed_gpu::shader_abi::MSL_PRELUDE, PRELUDE_COMMON, body);
    dev.create_shader(&ShaderDesc { label, stage, entry, code: ShaderCode::MslSource(&src) })
}

/// Capacities of a [`Renderer`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RendererDesc {
    pub max_instances: u32,
    pub max_vertices: u32,
    pub max_indices: u32,
    pub max_meshes: u32,
    pub max_materials: u32,
    pub max_nodes: u32,
    pub lod: LodConfig,
}

impl Default for RendererDesc {
    fn default() -> Self {
        Self {
            max_instances: 131_072,
            max_vertices: 1 << 20,
            max_indices: 1 << 22,
            max_meshes: 256,
            max_materials: 4096,
            max_nodes: 256,
            lod: LodConfig::default(),
        }
    }
}

/// What [`Renderer::flush_uploads`] sent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UploadStats {
    pub instance_ranges: u32,
    pub instance_bytes: u64,
    pub material_bytes: u64,
}

/// Renderer (M2). Create once per device; add meshes, materials and instances; call
/// [`Renderer::flush_uploads`] before `begin_frame` and [`Renderer::render`] between
/// `begin_frame` and `end_frame`; [`Renderer::destroy`] before dropping the device.
pub struct Renderer {
    desc: RendererDesc,
    shaders: Vec<ShaderId>,
    resolve: RenderPipelineId,
    tonemap_vs: ShaderId,
    tonemap_fs: ShaderId,
    tonemap: Vec<(Format, RenderPipelineId)>,
    pick_pipe: ComputePipelineId,
    pop_pipe: ComputePipelineId,
    vertices: Tracked,
    indices: Tracked,
    instance_buf: Tracked,
    materials_buf: Tracked,
    frame_data: Tracked,
    nodes_buf: Tracked,
    vertex_count: u32,
    index_count: u32,
    mesh_count: u32,
    materials: Vec<Material>,
    materials_dirty: bool,
    pools: InstancePools,
    /// Slots whose data has been flushed (the culling dispatch covers these).
    slots_flushed: u32,
    lod: DiscreteLod,
    pick_ring: Ring,
    pick_requests: [(u32, u32); provider::RING],
    pick: Option<PickResult>,
    pop_ring: Option<(Ring, u32, u32)>,
    pop_frames: Vec<PopFrame>,
    frame_counter: u32,
    pool: TransientPool,
    scratch: Vec<Timing>,
    bytes_scratch: Vec<u8>,
}

impl Renderer {
    pub fn new(dev: &dyn Device) -> Result<Self, ed_gpu::Error> {
        Self::with_desc(dev, &RendererDesc::default())
    }

    pub fn with_desc(dev: &dyn Device, desc: &RendererDesc) -> Result<Self, ed_gpu::Error> {
        let vs = shader(dev, "resolve vs", ShaderStage::Vertex, "vs_resolve", RESOLVE_SRC)?;
        let fs = shader(dev, "resolve fs", ShaderStage::Fragment, "fs_resolve", RESOLVE_SRC)?;
        let tonemap_vs = shader(dev, "tonemap vs", ShaderStage::Vertex, "vs_fullscreen", TONEMAP_SRC)?;
        let tonemap_fs = shader(dev, "tonemap fs", ShaderStage::Fragment, "fs_tonemap", TONEMAP_SRC)?;
        let pick_cs = shader(dev, "pick", ShaderStage::Compute, "pick", DEBUG_SRC)?;
        let pop_cs = shader(dev, "pop_stats", ShaderStage::Compute, "pop_stats", DEBUG_SRC)?;
        let resolve = dev.create_render_pipeline(&RenderPipelineDesc {
            label: "resolve",
            vertex: vs,
            fragment: Some(fs),
            color_formats: &[HDR_FORMAT],
            blend: Blend::Opaque,
            depth: None,
            cull: Cull::None,
            topology: Topology::Triangles,
            samples: 1,
        })?;
        let pick_pipe = dev.create_compute_pipeline(&ComputePipelineDesc { label: "pick", shader: pick_cs, workgroup_size: [1, 1, 1] })?;
        let pop_pipe = dev.create_compute_pipeline(&ComputePipelineDesc { label: "pop_stats", shader: pop_cs, workgroup_size: [8, 8, 1] })?;
        let upload = Usage { shader_read: true, copy_dst: true, ..Default::default() };
        let per_frame = Usage { shader_read: true, cpu_write: true, ..Default::default() };
        let vertices = Tracked::new(dev, "vertices", desc.max_vertices as u64 * (VERTEX_WORDS * 4) as u64, upload)?;
        let indices = Tracked::new(dev, "indices", desc.max_indices as u64 * 4, upload)?;
        let instance_buf = Tracked::new(dev, "instances", desc.max_instances as u64 * (INSTANCE_WORDS * 4) as u64, upload)?;
        let materials_buf = Tracked::new(dev, "materials", desc.max_materials as u64 * (MATERIAL_WORDS * 4) as u64, upload)?;
        let frame_data = Tracked::new(dev, "frame data", (FRAME_WORDS * 4) as u64, per_frame)?;
        let nodes_buf = Tracked::new(dev, "nodes", desc.max_nodes as u64 * 16, per_frame)?;
        Ok(Self {
            desc: *desc,
            shaders: vec![vs, fs, pick_cs, pop_cs],
            resolve,
            tonemap_vs,
            tonemap_fs,
            tonemap: Vec::new(),
            pick_pipe,
            pop_pipe,
            vertices,
            indices,
            instance_buf,
            materials_buf,
            frame_data,
            nodes_buf,
            vertex_count: 0,
            index_count: 0,
            mesh_count: 0,
            materials: Vec::new(),
            materials_dirty: false,
            pools: InstancePools::new(desc.max_instances, desc.max_nodes),
            slots_flushed: 0,
            lod: DiscreteLod::new(dev, desc.max_instances, desc.max_meshes, desc.lod)?,
            pick_ring: Ring::new(dev, "pick", 16)?,
            pick_requests: [(0, 0); provider::RING],
            pick: None,
            pop_ring: None,
            pop_frames: Vec::new(),
            frame_counter: 0,
            pool: TransientPool::new(),
            scratch: Vec::new(),
            bytes_scratch: Vec::new(),
        })
    }

    pub fn desc(&self) -> &RendererDesc {
        &self.desc
    }

    /// Adds a mesh and its LOD chain to the shared geometry buffers. The data lands at the
    /// next frame start; call outside a frame.
    pub fn add_mesh(&mut self, dev: &dyn Device, m: &MeshAsset) -> Result<MeshId, ed_gpu::Error> {
        let bad = |s: &str| Err(ed_gpu::Error::Invalid(format!("add_mesh: {s}")));
        if m.lods.is_empty() || m.lods.len() > MAX_LODS {
            return bad("needs 1..=4 LODs");
        }
        if m.normals.len() != m.positions.len() {
            return bad("normals and positions differ in length");
        }
        if self.mesh_count >= self.desc.max_meshes {
            return bad("mesh table full");
        }
        let nv = m.positions.len() as u32;
        let ni: u32 = m.lods.iter().map(|l| l.indices.len() as u32).sum();
        if self.vertex_count + nv > self.desc.max_vertices || self.index_count + ni > self.desc.max_indices {
            return bad("geometry buffers full");
        }
        let mut vw = Vec::with_capacity(nv as usize * VERTEX_WORDS);
        for (p, n) in m.positions.iter().zip(&m.normals) {
            vw.extend_from_slice(&[p[0].to_bits(), p[1].to_bits(), p[2].to_bits(), gpu::oct_encode(Vec3::from(*n))]);
        }
        let mut rec = MeshRecord {
            sphere: Vec4::ZERO,
            lod_count: m.lods.len() as u32,
            first_index: [0; MAX_LODS],
            index_count: [0; MAX_LODS],
            base_vertex: self.vertex_count as i32,
            error: [0.0; MAX_LODS],
        };
        let (c, r) = m.bounding_sphere();
        rec.sphere = c.extend(r);
        let mut iw = Vec::with_capacity(ni as usize);
        let mut first = self.index_count;
        for (l, lod) in m.lods.iter().enumerate() {
            if lod.indices.len() % 3 != 0 || lod.indices.iter().any(|&i| i >= nv) {
                return bad("indices must be triangles into the mesh's vertices");
            }
            rec.first_index[l] = first;
            rec.index_count[l] = lod.indices.len() as u32;
            rec.error[l] = lod.error;
            first += lod.indices.len() as u32;
            iw.extend_from_slice(&lod.indices);
        }
        dev.write_buffer(self.vertices.id, self.vertex_count as u64 * (VERTEX_WORDS * 4) as u64, &gpu::bytes(&vw))?;
        dev.write_buffer(self.indices.id, self.index_count as u64 * 4, &gpu::bytes(&iw))?;
        self.vertex_count += nv;
        self.index_count += ni;
        self.lod.add_mesh(rec);
        self.mesh_count += 1;
        Ok(MeshId(self.mesh_count - 1))
    }

    /// Adds a material row. `None` when the table is full.
    pub fn add_material(&mut self, m: Material) -> Option<MaterialId> {
        if self.materials.len() as u32 >= self.desc.max_materials {
            return None;
        }
        self.materials.push(m);
        self.materials_dirty = true;
        Some(MaterialId(self.materials.len() as u32 - 1))
    }
    pub fn set_material(&mut self, id: MaterialId, m: Material) {
        self.materials[id.0 as usize] = m;
        self.materials_dirty = true;
    }

    /// The instance pools (add, move, remove instances; node origins).
    pub fn instances(&mut self) -> &mut InstancePools {
        &mut self.pools
    }
    pub fn instance_pools(&self) -> &InstancePools {
        &self.pools
    }
    /// Provider 1 settings (LOD threshold, fade length, occlusion).
    pub fn lod_config(&mut self) -> &mut LodConfig {
        &mut self.lod.config
    }

    /// Sends changed instances (merged dirty ranges), materials and the provider's tables.
    /// Call before `begin_frame`: the data lands at the start of that frame. Cost is
    /// proportional to what changed.
    pub fn flush_uploads(&mut self, dev: &dyn Device) -> Result<UploadStats, ed_gpu::Error> {
        let mut st = UploadStats::default();
        for r in self.pools.take_dirty_ranges() {
            let b = gpu::bytes(self.pools.range_words(&r));
            dev.write_buffer(self.instance_buf.id, r.start as u64 * (INSTANCE_WORDS * 4) as u64, &b)?;
            st.instance_ranges += 1;
            st.instance_bytes += b.len() as u64;
        }
        self.slots_flushed = self.pools.slots();
        if self.materials_dirty {
            let w: Vec<u32> = self.materials.iter().flat_map(|m| m.words()).collect();
            let b = gpu::bytes(&w);
            dev.write_buffer(self.materials_buf.id, 0, &b)?;
            st.material_bytes = b.len() as u64;
            self.materials_dirty = false;
        }
        self.lod.flush(dev, &self.pools)?;
        Ok(st)
    }

    /// Pass names (timer labels) of a frame, in run order.
    pub fn pass_names(&self, params: &FrameParams) -> Vec<&'static str> {
        match params.mode {
            SceneMode::Empty => PASSES_EMPTY.to_vec(),
            SceneMode::Scene => {
                let mut v = self.lod.pass_names().to_vec();
                v.push("resolve");
                if params.pop_stats {
                    v.push("pop_stats");
                }
                if params.pick.is_some() {
                    v.push("pick");
                }
                v.extend(["tonemap", "overlay"]);
                v
            }
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

    /// Reads finished frames' counters, pick answers and pop tiles.
    pub fn poll(&mut self, dev: &dyn Device) {
        self.lod.poll(dev);
        let (reqs, pools, out) = (&self.pick_requests, &self.pools, &mut self.pick);
        self.pick_ring.poll(dev, &mut self.bytes_scratch, |frame, b| {
            let w = gpu::words(b);
            let (x, y) = reqs[(frame % provider::RING as u64) as usize];
            let instance = (w[0] != 0).then(|| w[0] - 1);
            let object = instance.and_then(|i| pools.object(i)).filter(|o| o.0 as u32 == w[1]);
            *out = Some(PickResult { frame, x, y, instance, object, lod: w[2] >> 28, primitive: w[2] & 0x0FFF_FFFF });
        });
        if let Some((ring, tx, ty)) = self.pop_ring.as_mut() {
            let (tx, ty) = (*tx, *ty);
            let frames = &mut self.pop_frames;
            ring.poll(dev, &mut self.bytes_scratch, |frame, b| {
                let w = gpu::words(b);
                let n = (tx * ty) as usize;
                let luma = (0..n).map(|i| f32::from_bits(w[i * 2])).collect();
                let transition_px = (0..n).map(|i| f32::from_bits(w[i * 2 + 1])).collect();
                frames.push(PopFrame { frame, tiles_x: tx, tiles_y: ty, luma, transition_px });
            });
        }
    }

    /// Record one view into `enc`. Call between `begin_frame` and `end_frame`.
    pub fn render(&mut self, dev: &dyn Device, enc: &mut dyn CommandEncoder, view: &View, params: &FrameParams) -> Result<(), GraphError> {
        self.poll(dev);
        let vp = view.viewport;
        let (w, h) = (vp.width.max(1), vp.height.max(1));
        let tonemap = self.tonemap_pipeline(dev, view.target.format)?;
        let frame_index = enc.frame_index();

        // Per-view data, camera-relative (P1-D7).
        let cam = &view.camera;
        let proj = reverse_z_projection(cam.fov_y, w as f32 / h as f32, cam.near);
        let view_m = cam.relative_view();
        let vpm = proj * view_m;
        let mut fd = [0u32; FRAME_WORDS];
        let put = |fd: &mut [u32], at: usize, v: &[f32]| {
            for (i, x) in v.iter().enumerate() {
                fd[at + i] = x.to_bits();
            }
        };
        put(&mut fd, 0, &vpm.to_cols_array());
        put(&mut fd, 16, &vpm.inverse().to_cols_array());
        put(&mut fd, 32, &view_m.to_cols_array());
        for (i, p) in frustum_planes(&vpm).iter().enumerate() {
            put(&mut fd, FD_PLANES + i * 4, &p.to_array());
        }
        let s = params.sun_dir.normalize_or(Vec3::Y);
        let rows: [[f32; 4]; 5] = [
            [s.x, s.y, s.z, 0.0],
            [3.0, 2.85, 2.6, 0.0],   // sun
            [0.18, 0.36, 0.85, 0.0], // zenith
            [0.62, 0.72, 0.86, 0.0], // horizon
            [0.24, 0.21, 0.17, 0.0], // ground
        ];
        for (i, r) in rows.iter().enumerate() {
            put(&mut fd, FD_SUN_DIR + i * 4, r);
        }
        put(&mut fd, FD_VIEWPORT, &[vp.x as f32, vp.y as f32, w as f32, h as f32]);
        let cfg = self.lod.config;
        put(&mut fd, FD_PARAMS, &[params.exposure, cfg.lod_px, cam.near, cfg.hysteresis]);
        put(&mut fd, FD_PROJ, &[proj.x_axis.x, proj.y_axis.y, w as f32, h as f32]);
        let scene = params.mode == SceneMode::Scene;
        if scene {
            self.lod.prepare(dev, &self.pools, w, h, &mut fd)?;
        }
        fd[gpu::FD_CULL + 2] = self.frame_counter;
        self.frame_counter = self.frame_counter.wrapping_add(1);
        let fd_bytes = gpu::bytes(&fd);
        dev.fill_frame_buffer(self.frame_data.id, &mut |dst| dst[..fd_bytes.len()].copy_from_slice(&fd_bytes))?;
        // The only use of the f64 camera position: one camera-relative origin per node.
        let nodes: Vec<u32> = self
            .pools
            .nodes()
            .iter()
            .flat_map(|n| {
                let r = (*n - cam.position).as_vec3();
                [r.x.to_bits(), r.y.to_bits(), r.z.to_bits(), 0]
            })
            .collect();
        let node_bytes = gpu::bytes(&nodes);
        dev.fill_frame_buffer(self.nodes_buf.id, &mut |dst| dst[..node_bytes.len()].copy_from_slice(&node_bytes))?;

        let encode_srgb = !matches!(view.target.format, Format::Rgba8Srgb | Format::Bgra8Srgb | Format::Rgba16Float);
        let exposure = params.exposure;
        let mut g = FrameGraph::new();
        let hdr = g.create_texture("hdr", TextureInfo { format: HDR_FORMAT, width: w, height: h });
        let out_info = TextureInfo { format: view.target.format, width: view.target.width, height: view.target.height };
        let out = g.import_texture("output", view.target.texture, out_info, view.target.initial, view.target.final_state);
        let fd_res = g.import_buffer("frame data", self.frame_data.id, self.frame_data.state, None);
        let mut imports = Vec::new();
        if scene {
            let cd = Some(ResourceState::CopyDst);
            let nodes_res = g.import_buffer("nodes", self.nodes_buf.id, self.nodes_buf.state, None);
            let verts = g.import_buffer("vertices", self.vertices.id, self.vertices.state, cd);
            let idx = g.import_buffer("indices", self.indices.id, self.indices.state, cd);
            let inst = g.import_buffer("instances", self.instance_buf.id, self.instance_buf.state, cd);
            let mats = g.import_buffer("materials", self.materials_buf.id, self.materials_buf.state, cd);
            imports = vec![nodes_res, verts, idx, inst, mats];
            let inputs = SharedInputs {
                frame: (fd_res, self.frame_data.bindless.0),
                instances: (inst, self.instance_buf.bindless.0),
                nodes: (nodes_res, self.nodes_buf.bindless.0),
                verts: (verts, self.vertices.bindless.0),
                indices: (idx, self.indices.bindless.0, self.indices.id),
                slots: self.slots_flushed,
            };
            let depth = g.create_texture("depth", TextureInfo { format: DEPTH_FORMAT, width: w, height: h });
            let vis = g.create_texture("visibility", TextureInfo { format: VIS_FORMAT, width: w, height: h });
            let po = self.lod.record(&mut g, frame_index, &inputs, VisTargets { depth, vis });
            let rc = constants(&[
                self.frame_data.bindless.0,
                0, // vis: bindless index of a transient, known in the pass
                self.instance_buf.bindless.0,
                self.nodes_buf.bindless.0,
                po.meshes.1,
                self.materials_buf.bindless.0,
                self.vertices.bindless.0,
                self.indices.bindless.0,
            ]);
            let resolve = self.resolve;
            g.add_pass("resolve", PassKind::Raster)
                .read(fd_res, ResourceState::ShaderRead)
                .read(vis, ResourceState::ShaderRead)
                .read(inst, ResourceState::ShaderRead)
                .read(nodes_res, ResourceState::ShaderRead)
                .read(po.meshes.0, ResourceState::ShaderRead)
                .read(mats, ResourceState::ShaderRead)
                .read(verts, ResourceState::ShaderRead)
                .read(idx, ResourceState::ShaderRead)
                .color(hdr, LoadOp::Discard, StoreOp::Store)
                .exec(move |ctx| {
                    let mut c = rc;
                    c[4..8].copy_from_slice(&ctx.bindless(vis).0.to_ne_bytes());
                    ctx.enc.set_render_pipeline(resolve);
                    ctx.enc.set_draw_constants(&c);
                    ctx.enc.draw(0..3, 0..1);
                });
            if params.pop_stats {
                let (tx, ty) = (w.div_ceil(POP_TILE), h.div_ceil(POP_TILE));
                if self.pop_ring.as_ref().map(|p| (p.1, p.2)) != Some((tx, ty)) {
                    if let Some((r, _, _)) = self.pop_ring.take() {
                        r.destroy(dev);
                    }
                    self.pop_ring = Some((Ring::new(dev, "pop tiles", tx as u64 * ty as u64 * 8)?, tx, ty));
                }
                let ring = &mut self.pop_ring.as_mut().expect("just created").0;
                let slot = ring.slot(frame_index);
                let b = &ring.bufs[slot];
                let res = g.import_buffer("pop tiles", b.id, b.state, Some(ResourceState::ShaderRead));
                imports.push(res);
                let (bi, pipe, state_bi) = (b.bindless.0, self.pop_pipe, po.state.1);
                g.add_pass("pop_stats", PassKind::Compute)
                    .read(hdr, ResourceState::ShaderRead)
                    .read(vis, ResourceState::ShaderRead)
                    .read(po.state.0, ResourceState::ShaderRead)
                    .write(res, ResourceState::ShaderWrite)
                    .exec(move |ctx| {
                        let (hi, vi) = (ctx.bindless(hdr).0, ctx.bindless(vis).0);
                        ctx.enc.set_compute_pipeline(pipe);
                        ctx.enc.set_compute_constants(&constants(&[hi, vi, state_bi, bi, w, h, tx, ty, exposure.to_bits()]));
                        ctx.enc.dispatch(tx.div_ceil(8), ty.div_ceil(8), 1);
                    });
            }
            if let Some((px, py)) = params.pick {
                let slot = self.pick_ring.slot(frame_index);
                self.pick_requests[slot] = (px, py);
                let b = &self.pick_ring.bufs[slot];
                let res = g.import_buffer("pick", b.id, b.state, Some(ResourceState::ShaderRead));
                imports.push(res);
                let (bi, pipe, inst_bi) = (b.bindless.0, self.pick_pipe, self.instance_buf.bindless.0);
                g.add_pass("pick", PassKind::Compute)
                    .read(vis, ResourceState::ShaderRead)
                    .read(inst, ResourceState::ShaderRead)
                    .write(res, ResourceState::ShaderWrite)
                    .exec(move |ctx| {
                        let vi = ctx.bindless(vis).0;
                        ctx.enc.set_compute_pipeline(pipe);
                        ctx.enc.set_compute_constants(&constants(&[vi, inst_bi, bi, px, py, w, h]));
                        ctx.enc.dispatch(1, 1, 1);
                    });
            }
        } else {
            g.add_pass("clear", PassKind::Raster).color(hdr, LoadOp::Clear([0.62, 0.72, 0.86, 1.0]), StoreOp::Store).done();
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
        if scene {
            self.lod.finish(&plan);
            let fs = |r: graph::Res| plan.final_states[r.index()];
            self.nodes_buf.state = fs(imports[0]);
            self.vertices.state = fs(imports[1]);
            self.indices.state = fs(imports[2]);
            self.instance_buf.state = fs(imports[3]);
            self.materials_buf.state = fs(imports[4]);
            let mut k = 5;
            if params.pop_stats {
                let slot = (frame_index % provider::RING as u64) as usize;
                if let Some((r, _, _)) = self.pop_ring.as_mut() {
                    r.bufs[slot].state = fs(imports[k]);
                }
                k += 1;
            }
            if params.pick.is_some() {
                let slot = (frame_index % provider::RING as u64) as usize;
                self.pick_ring.bufs[slot].state = fs(imports[k]);
            }
        }
        Ok(())
    }

    /// GPU culling counters of the latest finished frame.
    pub fn cull_stats(&self) -> Option<CullStats> {
        self.lod.stats()
    }
    /// Draw calls recorded per scene frame: the provider's indirect draws plus resolve and
    /// tonemap.
    pub fn draw_calls(&self) -> u32 {
        self.lod.draw_calls() + 2
    }
    /// CPU time of the last frame's multi-draw loops, ms.
    pub fn draw_loop_ms(&self) -> f64 {
        self.lod.draw_loop_ms()
    }
    /// Culling counters of every finished frame since the last call, oldest first.
    pub fn take_cull_history(&mut self) -> Vec<CullStats> {
        self.lod.take_history()
    }
    /// Provider 1, for its counters and settings.
    pub fn lod_provider(&self) -> &DiscreteLod {
        &self.lod
    }
    /// The latest pick answer.
    pub fn pick_result(&self) -> Option<PickResult> {
        self.pick
    }
    /// Pop-detector tiles of finished frames since the last call, oldest first.
    pub fn take_pop_frames(&mut self) -> Vec<PopFrame> {
        std::mem::take(&mut self.pop_frames)
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
        for t in [self.vertices, self.indices, self.instance_buf, self.materials_buf, self.frame_data, self.nodes_buf] {
            t.destroy(dev);
        }
        Box::new(self.lod).destroy(dev);
        self.pick_ring.destroy(dev);
        if let Some((r, _, _)) = self.pop_ring {
            r.destroy(dev);
        }
        dev.destroy_render_pipeline(self.resolve);
        dev.destroy_compute_pipeline(self.pick_pipe);
        dev.destroy_compute_pipeline(self.pop_pipe);
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
        let m = cube_mesh();
        let p = |i: u32| Vec3::from(m.positions[i as usize]);
        for t in m.lods[0].indices.chunks(3) {
            let n = Vec3::from(m.normals[t[0] as usize]);
            let face = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
            assert!(face.dot(n) > 0.0);
        }
    }

    #[test]
    fn frustum_planes_contain_the_view() {
        let c = Camera { position: DVec3::ZERO, yaw: 0.3, pitch: -0.2, fov_y: 1.0, near: 0.1 };
        let vp = c.relative_view_proj(16.0 / 9.0);
        let pl = frustum_planes(&vp);
        let inside = |p: Vec3, r: f32| pl.iter().all(|q| q.truncate().dot(p) + q.w >= -r);
        assert!(inside(c.forward() * 50.0, 0.0));
        assert!(!inside(-c.forward() * 50.0, 1.0), "behind");
        assert!(!inside(c.forward() * 50.0 + c.right() * 500.0, 1.0), "far right");
        assert!(inside(c.forward() * 50.0 + c.right() * 500.0, 600.0), "big sphere reaches in");
        assert!(!inside(c.forward() * 0.05, 0.01), "closer than near");
        assert!(inside(c.forward() * 1e7, 0.0), "no far plane");
    }
}
