//! egui painter on `ed_gpu` (P1-D14). Uses only the public RHI: one `cpu_write` buffer for
//! vertices and indices (one copy per frame in flight, pulled by the vertex shader through the
//! bindless table), one `Rgba8Unorm` texture per egui texture (gamma-space contents), a scissor per clip rectangle.
//!
//! Frame protocol:
//! 1. **Outside a frame** (before `Device::begin_frame`): [`Painter::prepare`] with the new
//!    [`PaintJob`]. Texture uploads are staged here (they land at the next frame start, the
//!    contract's upload rule) and the vertex ring grows if needed.
//! 2. **Inside the frame**: [`Painter::paint`] records one render pass into the caller's
//!    colour target (which must be in `RenderTarget`). With no new job, `paint` draws the last
//!    prepared geometry again, so an idle editor costs a buffer copy and a few draws.
//!
//! Colour follows egui's rules (vertex colours and textures are premultiplied sRGB, blending
//! is premultiplied): the shader works in gamma space, as egui expects, and converts to
//! linear only when the target stores linear values (`*Srgb` and float formats), like
//! `egui_wgpu`. On a `*Unorm` target blending happens in gamma space, exactly as egui intends.
//!
//! Textures freed by egui are destroyed after the next paint; `ed_gpu` defers the actual
//! release until every frame in flight that could sample them has finished.

use std::collections::HashMap;

use ed_gpu::*;

/// What egui produced for one frame. `textures` must reach [`Painter::prepare`] (or be
/// cleared with [`PaintJob::discard`]): egui asserts in debug builds that texture deltas are
/// never dropped unapplied.
#[derive(Debug)]
pub struct PaintJob {
    pub primitives: Vec<egui::ClippedPrimitive>,
    pub textures: egui::TexturesDelta,
    pub pixels_per_point: f32,
}

impl PaintJob {
    /// Drop the job without painting it.
    pub fn discard(mut self) {
        self.textures.clear();
    }
}

/// The colour target `paint` draws into.
#[derive(Clone, Copy, Debug)]
pub struct PaintTarget {
    pub texture: TextureId,
    pub format: Format,
    pub width: u32,
    pub height: u32,
    /// `Load` to draw over the scene; `Clear` for a standalone panel image.
    pub load: LoadOp,
}

/// Counters from the last `prepare`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaintStats {
    pub draws: u32,
    pub vertices: u32,
    pub indices: u32,
    pub textures: u32,
    pub texture_uploads: u32,
    pub ring_bytes: u64,
}

struct Tex {
    id: TextureId,
    bind: BindlessTexture,
    size: [usize; 2],
    /// CPU copy (RGBA8, premultiplied sRGB) so partial updates can re-upload the whole
    /// subresource (`write_texture` writes whole subresources).
    pixels: Vec<u8>,
    sampler: usize,
    /// Uploaded but not yet transitioned to `ShaderRead`.
    fresh: bool,
    owned: bool,
}

#[derive(Clone, Copy, Debug)]
struct Draw {
    clip: egui::Rect,
    vtx_byte: u32,
    idx_byte: u32,
    count: u32,
    tex: egui::TextureId,
}

pub const VERTEX_BYTES: usize = 20;
const SAMPLERS: [(Filter, AddressMode); 6] = [
    (Filter::Linear, AddressMode::Clamp),
    (Filter::Nearest, AddressMode::Clamp),
    (Filter::Linear, AddressMode::Repeat),
    (Filter::Nearest, AddressMode::Repeat),
    (Filter::Linear, AddressMode::Mirror),
    (Filter::Nearest, AddressMode::Mirror),
];

pub struct Painter {
    vs: ShaderId,
    fs: ShaderId,
    pipelines: Vec<(Format, RenderPipelineId)>,
    samplers: Vec<(SamplerId, BindlessSampler)>,
    textures: HashMap<egui::TextureId, Tex>,
    ring: Option<(BufferId, BindlessBuffer, u64)>,
    geometry: Vec<u8>,
    draws: Vec<Draw>,
    pixels_per_point: f32,
    pending_free: Vec<egui::TextureId>,
    /// Wrap `paint` in a GPU timer scope with this label.
    pub timer: Option<&'static str>,
    pub stats: PaintStats,
}

impl Painter {
    pub fn new(dev: &dyn Device) -> Result<Self> {
        let src = format!("{}\n{}", shader_abi::MSL_PRELUDE, SHADER);
        let vs = dev.create_shader(&ShaderDesc {
            label: "egui vs",
            stage: ShaderStage::Vertex,
            entry: "vs_egui",
            code: ShaderCode::MslSource(&src),
        })?;
        let fs = dev.create_shader(&ShaderDesc {
            label: "egui fs",
            stage: ShaderStage::Fragment,
            entry: "fs_egui",
            code: ShaderCode::MslSource(&src),
        })?;
        let mut samplers = Vec::new();
        for (f, a) in SAMPLERS {
            let s = dev.create_sampler(&SamplerDesc {
                min_mag: f,
                mip: Filter::Nearest,
                address: a,
                anisotropy: 1,
                compare: None,
            })?;
            samplers.push((s, dev.bind_sampler(s)?));
        }
        Ok(Painter {
            vs,
            fs,
            pipelines: Vec::new(),
            samplers,
            textures: HashMap::new(),
            ring: None,
            geometry: Vec::new(),
            draws: Vec::new(),
            pixels_per_point: 1.0,
            pending_free: Vec::new(),
            timer: Some("egui"),
            stats: PaintStats::default(),
        })
    }

    /// Use an application texture (already bound, kept in `ShaderRead` by its owner) as
    /// `egui::TextureId::User(id)`.
    pub fn set_user_texture(
        &mut self,
        id: u64,
        texture: TextureId,
        bind: BindlessTexture,
        size: [usize; 2],
    ) {
        self.textures.insert(
            egui::TextureId::User(id),
            Tex {
                id: texture,
                bind,
                size,
                pixels: Vec::new(),
                sampler: 0,
                fresh: false,
                owned: false,
            },
        );
    }

    /// Stage texture changes and the job's geometry. Call outside a frame.
    pub fn prepare(&mut self, dev: &dyn Device, job: &mut PaintJob) -> Result<()> {
        let mut uploads = 0;
        // Apply deltas in a fixed order (HashMap iteration order is not stable).
        let mut set: Vec<_> = job.textures.set.drain().collect();
        set.sort_by_key(|(id, _)| tex_key(*id));
        for (id, deltas) in set {
            for d in deltas {
                self.apply_delta(id, &d);
            }
            self.upload(dev, id)?;
            uploads += 1;
        }
        let mut free: Vec<_> = job.textures.free.drain().collect();
        free.sort_by_key(|id| tex_key(*id));
        self.pending_free.extend(free);

        // Geometry: all vertices, then all indices.
        self.geometry.clear();
        self.draws.clear();
        self.pixels_per_point = job.pixels_per_point;
        let meshes: Vec<(&egui::Rect, &egui::epaint::Mesh)> = job
            .primitives
            .iter()
            .filter_map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) if !m.indices.is_empty() => {
                    Some((&p.clip_rect, m))
                }
                _ => None, // paint callbacks are not supported
            })
            .collect();
        let nv: usize = meshes.iter().map(|(_, m)| m.vertices.len()).sum();
        let ni: usize = meshes.iter().map(|(_, m)| m.indices.len()).sum();
        self.geometry.reserve(nv * VERTEX_BYTES + ni * 4);
        let mut vtx_byte = 0u32;
        let mut offsets = Vec::with_capacity(meshes.len());
        for (_, m) in &meshes {
            offsets.push(vtx_byte);
            for v in &m.vertices {
                self.geometry.extend_from_slice(&v.pos.x.to_le_bytes());
                self.geometry.extend_from_slice(&v.pos.y.to_le_bytes());
                self.geometry.extend_from_slice(&v.uv.x.to_le_bytes());
                self.geometry.extend_from_slice(&v.uv.y.to_le_bytes());
                self.geometry.extend_from_slice(&v.color.to_array());
            }
            vtx_byte += (m.vertices.len() * VERTEX_BYTES) as u32;
        }
        for ((clip, m), vtx) in meshes.iter().zip(offsets) {
            let idx_byte = self.geometry.len() as u32;
            for i in &m.indices {
                self.geometry.extend_from_slice(&i.to_le_bytes());
            }
            self.draws.push(Draw {
                clip: **clip,
                vtx_byte: vtx,
                idx_byte,
                count: m.indices.len() as u32,
                tex: m.texture_id,
            });
        }

        let need = (self.geometry.len() as u64).max(64);
        if self.ring.is_none_or(|(_, _, cap)| cap < need) {
            if let Some((b, bind, _)) = self.ring.take() {
                dev.unbind_buffer(bind);
                dev.destroy_buffer(b);
            }
            let cap = need.next_power_of_two().max(256 * 1024);
            let b = dev.create_buffer(&BufferDesc {
                label: "egui ring",
                size: cap,
                usage: Usage {
                    shader_read: true,
                    cpu_write: true,
                    ..Default::default()
                },
            })?;
            self.ring = Some((b, dev.bind_buffer(b)?, cap));
        }
        self.stats = PaintStats {
            draws: self.draws.len() as u32,
            vertices: nv as u32,
            indices: ni as u32,
            textures: self.textures.len() as u32,
            texture_uploads: uploads,
            ring_bytes: self.ring.map_or(0, |r| r.2),
        };
        Ok(())
    }

    fn apply_delta(&mut self, id: egui::TextureId, d: &egui::epaint::ImageDelta) {
        let egui::ImageData::Color(img) = &d.image;
        let [w, h] = img.size;
        let sampler = sampler_index(&d.options);
        match d.pos {
            None => {
                let mut pixels = Vec::with_capacity(w * h * 4);
                for c in &img.pixels {
                    pixels.extend_from_slice(&c.to_array());
                }
                let fresh = Tex {
                    id: TextureId::from_parts(0, std::num::NonZeroU32::MIN),
                    bind: BindlessTexture(0),
                    size: [w, h],
                    pixels,
                    sampler,
                    fresh: false,
                    owned: false,
                };
                match self.textures.get_mut(&id) {
                    Some(t) if t.owned => {
                        t.size = fresh.size;
                        t.pixels = fresh.pixels;
                        t.sampler = sampler;
                    }
                    _ => {
                        self.textures.insert(id, fresh);
                    }
                }
            }
            Some([x0, y0]) => {
                let Some(t) = self.textures.get_mut(&id) else {
                    return;
                };
                let tw = t.size[0];
                for y in 0..h {
                    for x in 0..w {
                        let (dx, dy) = (x0 + x, y0 + y);
                        if dx < tw && dy < t.size[1] {
                            let o = (dy * tw + dx) * 4;
                            t.pixels[o..o + 4].copy_from_slice(&img.pixels[y * w + x].to_array());
                        }
                    }
                }
                t.sampler = sampler;
            }
        }
    }

    /// Recreate the GPU texture from the CPU copy. A new texture each time keeps the upload
    /// rule simple (the destination starts `Undefined`); the old one is released by the device
    /// once no frame in flight uses it.
    fn upload(&mut self, dev: &dyn Device, id: egui::TextureId) -> Result<()> {
        let Some(t) = self.textures.get_mut(&id) else {
            return Ok(());
        };
        if t.owned {
            dev.unbind_texture(t.bind);
            dev.destroy_texture(t.id);
        }
        let (w, h) = (t.size[0].max(1) as u32, t.size[1].max(1) as u32);
        let tex = dev.create_texture(&TextureDesc {
            label: "egui texture",
            kind: TextureKind::D2,
            format: Format::Rgba8Unorm,
            width: w,
            height: h,
            depth_or_layers: 1,
            mips: 1,
            samples: 1,
            usage: Usage {
                shader_read: true,
                copy_dst: true,
                ..Default::default()
            },
        })?;
        if t.pixels.len() == (w * h * 4) as usize {
            dev.write_texture(tex, Subresource { mip: 0, layer: 0 }, &t.pixels)?;
        } else {
            dev.write_texture(
                tex,
                Subresource { mip: 0, layer: 0 },
                &vec![0u8; (w * h * 4) as usize],
            )?;
        }
        t.id = tex;
        t.bind = dev.bind_texture(tex)?;
        t.owned = true;
        t.fresh = true;
        Ok(())
    }

    fn pipeline(&mut self, dev: &dyn Device, format: Format) -> Result<RenderPipelineId> {
        if let Some((_, p)) = self.pipelines.iter().find(|(f, _)| *f == format) {
            return Ok(*p);
        }
        let p = dev.create_render_pipeline(&RenderPipelineDesc {
            label: "egui",
            vertex: self.vs,
            fragment: Some(self.fs),
            color_formats: &[format],
            blend: Blend::Premultiplied,
            depth: None,
            cull: Cull::None,
            topology: Topology::Triangles,
            samples: 1,
        })?;
        self.pipelines.push((format, p));
        Ok(p)
    }

    /// Record the UI into `target` (in `RenderTarget`, left in `RenderTarget`). Draws the last
    /// prepared job; call `prepare` first when there is a new one.
    pub fn paint(
        &mut self,
        dev: &dyn Device,
        enc: &mut dyn CommandEncoder,
        target: &PaintTarget,
    ) -> Result<()> {
        let pso = self.pipeline(dev, target.format)?;
        let Some((ring, ring_bind, _)) = self.ring else {
            return Ok(());
        };
        if !self.geometry.is_empty() {
            let geo = &self.geometry;
            dev.fill_frame_buffer(ring, &mut |b| b[..geo.len()].copy_from_slice(geo))?;
        }
        let fresh: Vec<Transition> = self
            .textures
            .values_mut()
            .filter(|t| t.fresh)
            .map(|t| {
                t.fresh = false;
                Transition::Texture {
                    texture: t.id,
                    sub: None,
                    from: ResourceState::CopyDst,
                    to: ResourceState::ShaderRead,
                }
            })
            .collect();
        if !fresh.is_empty() {
            enc.transition(&fresh);
        }
        if let Some(l) = self.timer {
            enc.begin_timer(l);
        }
        enc.begin_render_pass(&RenderPassDesc {
            label: "egui",
            colors: &[Attachment {
                texture: target.texture,
                sub: Subresource { mip: 0, layer: 0 },
                load: target.load,
                store: StoreOp::Store,
            }],
            depth: None,
        });
        enc.set_viewport(0.0, 0.0, target.width as f32, target.height as f32);
        enc.set_render_pipeline(pso);
        let ppp = self.pixels_per_point.max(1e-3);
        let linear_out = !matches!(
            target.format,
            Format::Rgba8Unorm | Format::Bgra8Unorm | Format::Rgb10A2Unorm
        ) as u32;
        let screen = [target.width as f32 / ppp, target.height as f32 / ppp];
        for d in &self.draws {
            let Some(t) = self.textures.get(&d.tex) else {
                continue;
            };
            let x0 = (d.clip.min.x * ppp).round().clamp(0.0, target.width as f32) as u32;
            let y0 = (d.clip.min.y * ppp)
                .round()
                .clamp(0.0, target.height as f32) as u32;
            let x1 = (d.clip.max.x * ppp).round().clamp(0.0, target.width as f32) as u32;
            let y1 = (d.clip.max.y * ppp)
                .round()
                .clamp(0.0, target.height as f32) as u32;
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            enc.set_scissor(x0, y0, x1 - x0, y1 - y0);
            let words = [
                screen[0].to_bits(),
                screen[1].to_bits(),
                ring_bind.0,
                d.vtx_byte,
                d.idx_byte,
                t.bind.0,
                self.samplers[t.sampler].1.0,
                linear_out,
            ];
            let mut c = [0u8; DRAW_CONSTANTS_BYTES];
            for (i, w) in words.iter().enumerate() {
                c[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
            }
            enc.set_draw_constants(&c);
            enc.draw(0..d.count, 0..1);
        }
        enc.end_render_pass();
        if self.timer.is_some() {
            enc.end_timer();
        }
        for id in std::mem::take(&mut self.pending_free) {
            if let Some(t) = self.textures.remove(&id)
                && t.owned
            {
                dev.unbind_texture(t.bind);
                dev.destroy_texture(t.id);
            }
        }
        Ok(())
    }

    /// Release every GPU resource (call before dropping the device).
    pub fn destroy(mut self, dev: &dyn Device) {
        for (_, t) in self.textures.drain() {
            if t.owned {
                dev.unbind_texture(t.bind);
                dev.destroy_texture(t.id);
            }
        }
        if let Some((b, bind, _)) = self.ring.take() {
            dev.unbind_buffer(bind);
            dev.destroy_buffer(b);
        }
        for (s, b) in self.samplers.drain(..) {
            dev.unbind_sampler(b);
            dev.destroy_sampler(s);
        }
        for (_, p) in self.pipelines.drain(..) {
            dev.destroy_render_pipeline(p);
        }
        dev.destroy_shader(self.vs);
        dev.destroy_shader(self.fs);
    }

    /// Bytes of vertex and index data in the last prepared job.
    pub fn geometry_bytes(&self) -> usize {
        self.geometry.len()
    }
}

fn tex_key(id: egui::TextureId) -> (u8, u64) {
    match id {
        egui::TextureId::Managed(n) => (0, n),
        egui::TextureId::User(n) => (1, n),
    }
}

fn sampler_index(o: &egui::TextureOptions) -> usize {
    let wrap = match o.wrap_mode {
        egui::TextureWrapMode::ClampToEdge => 0,
        egui::TextureWrapMode::Repeat => 2,
        egui::TextureWrapMode::MirroredRepeat => 4,
    };
    wrap + usize::from(o.magnification == egui::TextureFilter::Nearest)
}

const SHADER: &str = r#"
struct EguiC { float2 screen; uint buf; uint vtx; uint idx; uint tex; uint smp; uint linear_out; uint pad[8]; };
struct EguiV { packed_float2 pos; packed_float2 uv; uint color; };
struct EguiOut { float4 pos [[position]]; float4 color; float2 uv; };

static float3 egui_linear_from_gamma(float3 s) {
    float3 lo = s / 12.92;
    float3 hi = pow((s + 0.055) / 1.055, float3(2.4));
    return select(hi, lo, s < 0.04045);
}

vertex EguiOut vs_egui(uint vid [[vertex_id]], ED_BINDLESS(EguiC)) {
    device const uchar* base = ed_buffers[ed_c.buf].p;
    uint i = ((device const uint*)(base + ed_c.idx))[vid];
    EguiV v = ((device const EguiV*)(base + ed_c.vtx))[i];
    EguiOut o;
    o.pos = float4(2.0 * v.pos.x / ed_c.screen.x - 1.0, 1.0 - 2.0 * v.pos.y / ed_c.screen.y, 0.0, 1.0);
    o.color = unpack_unorm4x8_to_float(v.color); // premultiplied, gamma
    o.uv = float2(v.uv);
    return o;
}

fragment float4 fs_egui(EguiOut in [[stage_in]], ED_BINDLESS(EguiC)) {
    // Textures hold egui's gamma-space premultiplied colours in a Unorm format, so sampling
    // returns gamma values and filtering happens in gamma space, as egui expects.
    float4 c = in.color * ed_tex2d(ed_c.tex).sample(ed_smp(ed_c.smp), in.uv);
    if (ed_c.linear_out != 0) {
        c.rgb = egui_linear_from_gamma(c.rgb);
    }
    return c;
}
"#;
