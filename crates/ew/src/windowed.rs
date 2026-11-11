//! Windowed shell (W1): winit window, Metal surface through `ed_gpu`, free camera. Starts in
//! the M2 proving ground (`--scene test` for the six-box scene). Right click picks.
//!
//! Loop per redraw: gather input -> fixed 60 Hz steps (accumulator, clamped) -> build the
//! frame with the interpolation fraction -> acquire, render, present.
//! Resize is applied between frames with `configure_surface` (no `wait_idle`): the frame
//! graph allocates new-size transients and evicts the old ones a few frames later.

use crate::args::{Args, SceneKind};
use crate::camera::{CameraInput, FreeCamera, Pose};
use crate::fixed_step::FixedStep;
use ed_gpu::{Backend, Device, DeviceDesc, Format, ResourceState, SurfaceConfig, SurfaceId, WindowHandle};
use ed_render::{FrameParams, Renderer, TestScene, View, ViewTarget, Viewport};
use glam::{Vec2, Vec3};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::collections::BTreeSet;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

pub const SURFACE_FORMAT: Format = Format::Bgra8Srgb;

struct Gpu {
    // Field order matters only for documentation; teardown is explicit in `shutdown`.
    renderer: Option<Renderer>,
    surface: SurfaceId,
    config: SurfaceConfig,
    window: Window,
}

struct App {
    args: Args,
    dev: Box<dyn Device>,
    gpu: Option<Gpu>,
    camera: FreeCamera,
    fixed: FixedStep,
    keys: BTreeSet<KeyCode>,
    look: Vec2,
    looking: bool,
    pending_size: Option<PhysicalSize<u32>>,
    last: Option<Instant>,
    presented: u64,
    lost: u64,
    errors: u64,
    log_t: Instant,
    log_frames: u64,
    log_cpu_ms: f64,
    max_wall_ms: f64,
    result: Result<(), String>,
    done: bool,
    resize_test: bool,
    cursor: (f64, f64),
    pick: Option<(u32, u32)>,
    last_pick_frame: u64,
    content: Option<crate::content::Content>,
    hot: crate::hot::HotReload,
}

pub fn run(args: Args) -> Result<(), String> {
    let dev = ed_gpu::create_device(Backend::Metal, &DeviceDesc { validation: args.validation, frames_in_flight: args.frames_in_flight })
        .map_err(|e| format!("device: {e}"))?;
    println!("ew: {}", crate::caps_line(dev.caps()));
    let event_loop = EventLoop::new().map_err(|e| format!("event loop: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        camera: match args.scene() {
            SceneKind::Test => FreeCamera::new(Pose::from_camera(&TestScene::test_camera())),
            SceneKind::Proving => FreeCamera { speed: 12.0, fast_speed: 120.0, ..FreeCamera::new(Pose::from_camera(&crate::proving::street_camera())) },
        },
        cursor: (0.0, 0.0),
        pick: None,
        last_pick_frame: u64::MAX,
        content: None,
        hot: crate::hot::HotReload::new(),
        fixed: FixedStep::default(),
        keys: BTreeSet::new(),
        look: Vec2::ZERO,
        looking: false,
        pending_size: None,
        last: None,
        presented: 0,
        lost: 0,
        errors: 0,
        log_t: Instant::now(),
        log_frames: 0,
        log_cpu_ms: 0.0,
        max_wall_ms: 0.0,
        result: Ok(()),
        done: false,
        resize_test: std::env::var("EW_RESIZE_TEST").is_ok_and(|v| v == "1"),
        gpu: None,
        dev,
        args,
    };
    event_loop.run_app(&mut app).map_err(|e| format!("event loop: {e}"))?;
    app.shutdown();
    app.result
}

impl App {
    fn create_gpu(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        let attrs = Window::default_attributes()
            .with_title("ew (EveryWorld, M3)")
            .with_inner_size(PhysicalSize::new(self.args.width, self.args.height));
        let window = el.create_window(attrs).map_err(|e| format!("window: {e}"))?;
        let ns_view = match window.window_handle().map_err(|e| format!("window handle: {e}"))?.as_raw() {
            RawWindowHandle::AppKit(h) => h.ns_view.as_ptr(),
            other => return Err(format!("unsupported window handle {other:?}")),
        };
        let size = window.inner_size();
        let config = SurfaceConfig { width: size.width.max(1), height: size.height.max(1), format: SURFACE_FORMAT, vsync: self.args.vsync, hdr: false };
        let surface = self
            .dev
            .create_surface(WindowHandle { window: ns_view, display: std::ptr::null_mut() }, &config)
            .map_err(|e| format!("create_surface: {e}"))?;
        println!(
            "ew: surface configured {}x{} {:?} vsync={} (scale factor {})",
            config.width,
            config.height,
            config.format,
            if config.vsync { "on" } else { "off (uncapped)" },
            window.scale_factor()
        );
        let mut renderer = Renderer::new(&*self.dev).map_err(|e| format!("renderer: {e}"))?;
        self.args.apply_lod(renderer.lod_config());
        match self.args.scene() {
            SceneKind::Test => {
                TestScene::load(&mut renderer, &*self.dev).map_err(|e| format!("test scene: {e}"))?;
            }
            SceneKind::Proving => {
                let c = crate::content::Content::load(&mut renderer, &*self.dev, &self.args.packs)?;
                println!(
                    "ew: proving ground from packs: {} placements -> {} instances ({} skinned), {} meshes, {} materials, {} textures, loaded in {:.0} ms",
                    c.placement_count(),
                    c.instance_count(),
                    c.skinned_parts(),
                    c.mesh_count(),
                    c.material_count(),
                    c.texture_count(),
                    c.load_ms
                );
                print!("{}", c.describe());
                self.content = Some(c);
            }
        }
        self.gpu = Some(Gpu { renderer: Some(renderer), surface, config, window });
        Ok(())
    }

    fn reconfigure(&mut self, size: PhysicalSize<u32>, vsync: bool) {
        let Some(gpu) = &mut self.gpu else { return };
        let cfg = SurfaceConfig { width: size.width.max(1), height: size.height.max(1), vsync, ..gpu.config.clone() };
        if (cfg.width, cfg.height, cfg.vsync) == (gpu.config.width, gpu.config.height, gpu.config.vsync) {
            return;
        }
        match self.dev.configure_surface(gpu.surface, &cfg) {
            Ok(()) => {
                println!("ew: surface configured {}x{} vsync={}", cfg.width, cfg.height, if vsync { "on" } else { "off (uncapped)" });
                gpu.config = cfg;
            }
            Err(e) => {
                eprintln!("ew: configure_surface failed: {e}");
                self.errors += 1;
            }
        }
    }

    fn input(&self) -> CameraInput {
        let k = |c| if self.keys.contains(&c) { 1.0 } else { 0.0 };
        CameraInput {
            axes: Vec3::new(k(KeyCode::KeyD) - k(KeyCode::KeyA), k(KeyCode::KeyE) - k(KeyCode::KeyQ), k(KeyCode::KeyW) - k(KeyCode::KeyS)),
            look: Vec2::ZERO,
            fast: self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight),
        }
    }

    fn frame(&mut self, el: &ActiveEventLoop) {
        if let Some(size) = self.pending_size.take() {
            let vsync = self.gpu.as_ref().map(|g| g.config.vsync).unwrap_or(self.args.vsync);
            self.reconfigure(size, vsync);
        }
        // Input -> fixed steps.
        let now = Instant::now();
        let frame_dt = self.last.map(|l| (now - l).as_secs_f64()).unwrap_or(0.0);
        if self.presented > 2 {
            self.max_wall_ms = self.max_wall_ms.max(frame_dt * 1e3);
        }
        self.last = Some(now);
        let base = self.input();
        let look = self.look;
        let mut used_look = false;
        let alpha = self.fixed.run(frame_dt, &mut self.camera, |i| {
            if i == 0 {
                used_look = true;
                CameraInput { look, ..base }
            } else {
                base
            }
        });
        if used_look {
            self.look = Vec2::ZERO;
        }
        let camera = self.camera.interpolated(alpha);

        let Some(gpu) = &mut self.gpu else { return };
        let dev = &*self.dev;
        // Uploads land at the start of the next frame: flush before begin_frame.
        let tf = Instant::now();
        let renderer = gpu.renderer.as_mut().expect("renderer lives until shutdown");
        // Hot reload (shaders, packs) and the character pose, before the uploads.
        let ev = self.hot.poll(renderer, dev, self.content.as_mut(), false);
        crate::hot::report(&ev);
        if matches!(ev.shaders, Some(Err(_))) || matches!(ev.packs, Some(Err(_))) {
            self.errors += 1;
        }
        if let Some(c) = &self.content
            && let Err(e) = c.animate(renderer, self.fixed.steps() as f64 / 60.0)
        {
            eprintln!("ew: {e}");
            self.errors += 1;
        }
        if let Err(e) = renderer.flush_uploads(dev) {
            eprintln!("ew: flush_uploads: {e}");
            self.errors += 1;
        }
        let flush_ms = tf.elapsed().as_secs_f64() * 1e3;
        let mut enc = match dev.begin_frame() {
            Ok(e) => e,
            Err(e) => {
                self.result = Err(format!("begin_frame: {e}"));
                el.exit();
                return;
            }
        };
        let tex = match dev.acquire(gpu.surface) {
            Ok(t) => t,
            Err(e) => {
                // The frame must still be ended; reconfigure before the next one.
                let _ = dev.end_frame(enc);
                self.lost += 1;
                if !matches!(e, ed_gpu::Error::SurfaceLost) {
                    eprintln!("ew: acquire failed: {e}");
                    self.errors += 1;
                }
                self.pending_size = Some(gpu.window.inner_size());
                return;
            }
        };
        // CPU time to build and submit, excluding the begin_frame and acquire waits.
        let t0 = Instant::now();
        let (w, h) = (gpu.config.width, gpu.config.height);
        let view = View {
            camera,
            viewport: Viewport { x: 0, y: 0, width: w, height: h },
            target: ViewTarget { texture: tex, format: gpu.config.format, width: w, height: h, initial: ResourceState::Undefined, final_state: Some(ResourceState::Present) },
        };
        let renderer = gpu.renderer.as_mut().expect("renderer lives until shutdown");
        let pick = self.pick.take().filter(|&(x, y)| x < w && y < h);
        if let Err(e) = renderer.render(dev, &mut *enc, &view, &FrameParams { pick, ..Default::default() }) {
            eprintln!("ew: render: {e}");
            self.errors += 1;
        }
        match dev.end_frame(enc) {
            Ok(()) => self.presented += 1,
            Err(e) => {
                eprintln!("ew: end_frame: {e}");
                self.errors += 1;
            }
        }
        self.log_cpu_ms += flush_ms + t0.elapsed().as_secs_f64() * 1e3;
        if let Some(p) = renderer.pick_result()
            && p.frame != self.last_pick_frame
        {
            self.last_pick_frame = p.frame;
            println!(
                "ew: pick ({}, {}): instance {:?}, object {}, lod {}",
                p.x,
                p.y,
                p.instance,
                p.object.map(|o| format!("{:#x}", o.0)).unwrap_or_else(|| "none (sky)".into()),
                p.lod
            );
        }
        self.log_frames += 1;

        let since = self.log_t.elapsed().as_secs_f64();
        if since >= 1.0 {
            let gpu_ms = renderer.timings(dev).map(|t| format!("{:.3}", t.frame_ms)).unwrap_or_else(|| "-".into());
            let cull = renderer
                .cull_stats()
                .map(|c| format!(", instances {} / frustum {} / occlusion {} / drawn {}, {} tris", c.total, c.after_frustum, c.after_occlusion, c.drawn(), c.triangles))
                .unwrap_or_default();
            println!(
                "ew: {:.1} fps, cpu {:.3} ms/frame, gpu {gpu_ms} ms (last retired frame), {} presented, steps {}{cull}",
                self.log_frames as f64 / since,
                self.log_cpu_ms / self.log_frames.max(1) as f64,
                self.presented,
                self.fixed.steps()
            );
            let _ = renderer.take_cull_history();
            self.log_t = Instant::now();
            self.log_frames = 0;
            self.log_cpu_ms = 0.0;
        }
        // `EW_RESIZE_TEST=1`: resize the window every 60 frames (resize self-test, logs the
        // worst frame interval at shutdown).
        if self.resize_test && self.presented.is_multiple_of(60) {
            let s = gpu.window.inner_size();
            let next = if s.width >= self.args.width { PhysicalSize::new(s.width * 3 / 4, s.height * 3 / 4) } else { PhysicalSize::new(self.args.width, self.args.height) };
            println!("ew: resize test: requesting {}x{}", next.width, next.height);
            let _ = gpu.window.request_inner_size(next);
        }
        if let Some(n) = self.args.frames.filter(|&n| n > 0)
            && self.presented >= n
        {
            println!("ew: {n} frames presented, exiting");
            el.exit();
        }
    }

    /// Clean shutdown: wait for the GPU, destroy everything the app created.
    fn shutdown(&mut self) {
        if self.done {
            return;
        }
        self.done = true;
        self.dev.wait_idle();
        if let Some(mut gpu) = self.gpu.take() {
            if let Some(r) = gpu.renderer.take() {
                r.destroy(&*self.dev);
            }
            self.dev.destroy_surface(gpu.surface);
            self.dev.wait_idle();
            drop(gpu.window);
        }
        println!(
            "ew: clean shutdown: {} frames presented, {} surface-lost frames, {} errors, worst frame interval {:.2} ms, allocated_bytes {}",
            self.presented,
            self.lost,
            self.errors,
            self.max_wall_ms,
            self.dev.allocated_bytes()
        );
        if self.errors > 0 && self.result.is_ok() {
            self.result = Err(format!("{} GPU/surface errors", self.errors));
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_none()
            && let Err(e) = self.create_gpu(el)
        {
            self.result = Err(e);
            el.exit();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => self.pending_size = Some(size),
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else { return };
                match event.state {
                    ElementState::Pressed => {
                        if code == KeyCode::Escape {
                            el.exit();
                        } else if code == KeyCode::KeyV
                            && !event.repeat
                            && let Some(gpu) = &self.gpu
                        {
                            let size = PhysicalSize::new(gpu.config.width, gpu.config.height);
                            let vsync = !gpu.config.vsync;
                            // Applied now: we are between frames here.
                            self.reconfigure(size, vsync);
                        }
                        self.keys.insert(code);
                    }
                    ElementState::Released => {
                        self.keys.remove(&code);
                    }
                }
            }
            WindowEvent::MouseInput { button: MouseButton::Left, state, .. } => self.looking = state == ElementState::Pressed,
            WindowEvent::MouseInput { button: MouseButton::Right, state: ElementState::Pressed, .. } => {
                self.pick = Some((self.cursor.0.max(0.0) as u32, self.cursor.1.max(0.0) as u32));
            }
            WindowEvent::CursorMoved { position, .. } => self.cursor = (position.x, position.y),
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.looking = false;
            }
            WindowEvent::RedrawRequested if !el.exiting() => self.frame(el),
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event
            && self.looking
        {
            self.look += Vec2::new(delta.0 as f32, delta.1 as f32);
        }
    }

    /// Frames are driven from here, not from `request_redraw`: on macOS redraw requests are
    /// paced by AppKit at the display rate, which would cap `--vsync off`. Pacing comes from
    /// `begin_frame` (frames in flight) and, with vsync on, from the drawable queue.
    /// `RedrawRequested` still renders, for the OS-driven redraws during a live resize.
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() && !el.exiting() {
            self.frame(el);
        }
    }

    fn exiting(&mut self, _el: &ActiveEventLoop) {
        self.shutdown();
    }
}
