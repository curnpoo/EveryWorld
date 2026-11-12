//! Offscreen session harness (M4b): the real `ew` host (device, renderer, pack content,
//! [`Session`]) driven frame by frame with synthetic input, for the editor host test and the
//! M4 captures. Every frame advances exactly one fixed step.

use crate::args::Args;
use crate::content::Content;
use crate::headless::{OUT_FORMAT, write_png};
use crate::session::{FrameIn, FrameOut, Mode, Session};
use ed_editor::editor::InputEvent;
use ed_gpu::{Backend, BufferDesc, BufferId, Device, DeviceDesc, ResourceState, TextureDesc, TextureId, TextureKind, Usage};
use ed_render::{FrameParams, Renderer, ViewTarget};
use std::path::{Path, PathBuf};

pub struct Harness {
    pub dev: Box<dyn Device>,
    renderer: Option<Renderer>,
    pub content: Content,
    pub session: Session,
    target: TextureId,
    readback: BufferId,
    pub width: u32,
    pub height: u32,
    pub frames: u64,
    destroyed: bool,
}

impl Harness {
    pub fn new(packs: &Path, layers: PathBuf, width: u32, height: u32, mode: Mode, editor_ui: bool, validation: bool) -> Result<Harness, String> {
        let dev = ed_gpu::create_device(Backend::Metal, &DeviceDesc { validation, frames_in_flight: 2 }).map_err(|e| format!("device: {e}"))?;
        let mut r = Renderer::new(&*dev).map_err(|e| format!("renderer: {e}"))?;
        Args::default().apply_lod(r.lod_config());
        let mut content = Content::load(&mut r, &*dev, packs)?;
        let session = Session::new(&mut r, &*dev, &mut content, layers, mode, editor_ui)?;
        let target = dev
            .create_texture(&TextureDesc {
                label: "harness output",
                kind: TextureKind::D2,
                format: OUT_FORMAT,
                width,
                height,
                depth_or_layers: 1,
                mips: 1,
                samples: 1,
                usage: Usage { render_target: true, copy_src: true, ..Default::default() },
            })
            .map_err(|e| format!("target: {e}"))?;
        let readback = dev
            .create_buffer(&BufferDesc { label: "harness readback", size: OUT_FORMAT.copy_row_pitch(width) as u64 * height as u64, usage: Usage { copy_dst: true, cpu_read: true, ..Default::default() } })
            .map_err(|e| format!("readback: {e}"))?;
        Ok(Harness { dev, renderer: Some(r), content, session, target, readback, width, height, frames: 0, destroyed: false })
    }

    pub fn renderer(&mut self) -> &mut Renderer {
        self.renderer.as_mut().expect("renderer")
    }

    /// [`Content::move_object`] with the harness renderer.
    pub fn move_object(&mut self, id: ed_geo::ObjectId, xf: &glam::Affine3A) -> bool {
        let r = self.renderer.as_mut().expect("renderer");
        self.content.move_object(r, id, xf)
    }

    /// One frame with these editor events (and otherwise idle input).
    pub fn frame(&mut self, events: Vec<InputEvent>) -> Result<FrameOut, String> {
        self.frame_with(FrameIn { events, ..Default::default() }, None)
    }

    /// One frame; `capture` also writes the final image (UI included) as a PNG.
    pub fn frame_with(&mut self, mut inp: FrameIn, capture: Option<&Path>) -> Result<FrameOut, String> {
        let dev = &*self.dev;
        let r = self.renderer.as_mut().expect("renderer");
        inp.dt = self.session.fixed.dt();
        inp.time = self.frames as f64 / 60.0;
        inp.size = [self.width, self.height];
        inp.pixels_per_point = 1.0;
        let out = self.session.update(r, dev, &mut self.content, inp)?;
        self.content.animate(r, self.session.fixed.steps() as f64 / 60.0)?;
        r.flush_uploads(dev).map_err(|e| format!("flush_uploads: {e}"))?;
        let mut enc = dev.begin_frame().map_err(|e| format!("begin_frame: {e}"))?;
        let target = ViewTarget { texture: self.target, format: OUT_FORMAT, width: self.width, height: self.height, initial: ResourceState::Undefined, final_state: None };
        self.session.record(r, dev, &mut *enc, target, FrameParams::default(), capture.map(|_| self.readback))?;
        dev.end_frame(enc).map_err(|e| format!("end_frame: {e}"))?;
        if let Some(t) = r.timings(dev) {
            self.session.set_timings(&t);
        }
        self.session.after_frame(r, dev);
        self.frames += 1;
        if let Some(path) = capture {
            dev.wait_idle();
            r.poll(dev);
            self.session.after_frame(r, dev);
            let mut raw = Vec::new();
            dev.read_buffer(self.readback, &mut raw).map_err(|e| format!("read_buffer: {e}"))?;
            let pitch = OUT_FORMAT.copy_row_pitch(self.width) as usize;
            let mut rgba = Vec::with_capacity((self.width * self.height * 4) as usize);
            for y in 0..self.height as usize {
                rgba.extend_from_slice(&raw[y * pitch..y * pitch + self.width as usize * 4]);
            }
            if let Some(d) = path.parent() {
                std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
            }
            write_png(path, self.width, self.height, &rgba)?;
        }
        Ok(out)
    }

    /// Renders and writes a PNG.
    pub fn capture(&mut self, path: &Path) -> Result<FrameOut, String> {
        self.frame_with(FrameIn::default(), Some(path))
    }

    pub fn destroy(mut self) {
        self.teardown();
    }

    fn teardown(&mut self) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;
        self.dev.wait_idle();
        self.session.destroy(&*self.dev);
        if let Some(mut r) = self.renderer.take() {
            self.content.clear(&mut r);
            r.destroy(&*self.dev);
        }
        self.dev.destroy_texture(self.target);
        self.dev.destroy_buffer(self.readback);
        self.dev.wait_idle();
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.teardown();
    }
}
