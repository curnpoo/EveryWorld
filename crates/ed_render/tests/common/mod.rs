//! Offscreen helpers for the renderer GPU tests.
#![allow(dead_code)]

use ed_gpu::*;
use ed_render::*;

pub const W: u32 = 256;
pub const H: u32 = 256;
pub const OUT_FORMAT: Format = Format::Rgba8Srgb;

pub struct Rig {
    pub dev: Box<dyn Device>,
    pub renderer: Option<Renderer>,
    pub target: TextureId,
    pub readback: BufferId,
}

impl Rig {
    /// A rig with the test scene loaded.
    pub fn new() -> Self {
        let mut rig = Self::bare();
        let dev = &*rig.dev;
        TestScene::load(rig.renderer.as_mut().unwrap(), dev).expect("test scene");
        rig
    }

    /// A rig with an empty renderer.
    pub fn bare() -> Self {
        let dev = create_device(Backend::Metal, &DeviceDesc { validation: true, frames_in_flight: 2 }).expect("Metal device");
        let renderer = Renderer::new(&*dev).expect("renderer");
        let target = dev
            .create_texture(&TextureDesc {
                label: "test output",
                kind: TextureKind::D2,
                format: OUT_FORMAT,
                width: W,
                height: H,
                depth_or_layers: 1,
                mips: 1,
                samples: 1,
                usage: Usage { render_target: true, copy_src: true, ..Default::default() },
            })
            .unwrap();
        let readback = dev
            .create_buffer(&BufferDesc {
                label: "test readback",
                size: (OUT_FORMAT.copy_row_pitch(W) * H) as u64,
                usage: Usage { copy_dst: true, cpu_read: true, ..Default::default() },
            })
            .unwrap();
        Self { dev, renderer: Some(renderer), target, readback }
    }

    pub fn view(&self, camera: Camera) -> View {
        View {
            camera,
            viewport: Viewport { x: 0, y: 0, width: W, height: H },
            target: ViewTarget { texture: self.target, format: OUT_FORMAT, width: W, height: H, initial: ResourceState::Undefined, final_state: None },
        }
    }

    /// Renders one frame; with `read`, copies the output to the readback buffer.
    pub fn frame(&mut self, camera: Camera, mode: SceneMode, read: bool) {
        let params = FrameParams { mode, readback: read.then_some(self.readback), ..Default::default() };
        self.frame_with(camera, &params);
    }

    /// Renders one frame with explicit parameters (uploads flushed first).
    pub fn frame_with(&mut self, camera: Camera, params: &FrameParams) {
        let view = self.view(camera);
        let params = *params;
        self.renderer.as_mut().unwrap().flush_uploads(&*self.dev).expect("flush");
        let mut e = self.dev.begin_frame().unwrap();
        self.renderer.as_mut().unwrap().render(&*self.dev, &mut *e, &view, &params).expect("render");
        self.dev.end_frame(e).expect("end_frame (validation)");
    }

    /// Renders the test scene and returns its W x H RGBA8 (sRGB) pixels.
    pub fn image(&mut self, camera: Camera) -> Vec<[u8; 4]> {
        self.frame(camera, SceneMode::Scene, true);
        self.read_image()
    }

    /// Waits for the GPU and returns the readback buffer as W x H RGBA8 pixels.
    pub fn read_image(&mut self) -> Vec<[u8; 4]> {
        self.dev.wait_idle();
        let mut raw = Vec::new();
        self.dev.read_buffer(self.readback, &mut raw).unwrap();
        let pitch = OUT_FORMAT.copy_row_pitch(W) as usize;
        let mut px = Vec::with_capacity((W * H) as usize);
        for y in 0..H as usize {
            for x in 0..W as usize {
                let o = y * pitch + x * 4;
                px.push([raw[o], raw[o + 1], raw[o + 2], raw[o + 3]]);
            }
        }
        px
    }

    /// Waits for the GPU and reads back every finished frame's results.
    pub fn settle(&mut self) {
        self.dev.wait_idle();
        self.renderer.as_mut().unwrap().poll(&*self.dev);
    }

    pub fn r(&mut self) -> &mut Renderer {
        self.renderer.as_mut().unwrap()
    }

    pub fn destroy(mut self) {
        self.dev.wait_idle();
        self.renderer.take().unwrap().destroy(&*self.dev);
        self.dev.destroy_texture(self.target);
        self.dev.destroy_buffer(self.readback);
        self.dev.wait_idle();
    }
}

pub fn at(px: &[[u8; 4]], x: u32, y: u32) -> [u8; 4] {
    px[(y * W + x) as usize]
}

/// FNV-1a over the pixels.
pub fn hash(px: &[[u8; 4]]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for p in px {
        for b in p {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}
