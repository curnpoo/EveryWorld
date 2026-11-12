//! Windowed shell (W1): winit window, Metal surface through `ed_gpu`. Starts in the proving
//! ground with the M4b session (`--scene test` for the six-box scene, free camera, right click
//! picks):
//! - Tab / F1 switch between game mode (WASD runs the character, Shift sprints, Space jumps,
//!   the mouse turns the follow camera; the cursor is captured) and editor mode (WASD/QE fly,
//!   right-drag looks, left click picks / drags the gizmo, Cmd+S saves the layers, Cmd+Z undo);
//! - C toggles the collision wireframe, I the top-down inset view, V vsync, Esc quits.
//!
//! Keys and the pointer go to the editor UI first; the camera only uses them when the UI does
//! not want them (`wants_pointer` / `wants_keyboard`).
//!
//! Loop per redraw: gather input -> fixed 60 Hz steps (accumulator, clamped) -> build the
//! frame with the interpolation fraction -> acquire, render, present.
//! Resize is applied between frames with `configure_surface` (no `wait_idle`): the frame
//! graph allocates new-size transients and evicts the old ones a few frames later.

use crate::args::{Args, SceneKind};
use crate::camera::{CameraInput, FreeCamera, Pose};
use crate::fixed_step::FixedStep;
use crate::session::{FrameIn, Mode, Session};
use ed_editor::editor::InputEvent;
use ed_editor::egui;
use ed_gpu::{Device, DeviceDesc, Format, ResourceState, SurfaceConfig, SurfaceId, WindowHandle};
use ed_render::{FrameParams, Renderer, TestScene, View, ViewTarget, Viewport};

use glam::{Vec2, Vec3};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::collections::BTreeSet;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

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
    /// M4b: character, editor, mode (proving ground only).
    session: Option<Session>,
    events: Vec<InputEvent>,
    ui_pointer: bool,
    ui_keyboard: bool,
    t_start: Instant,
}

pub fn run(args: Args) -> Result<(), String> {
    if args.scene() == SceneKind::Proving {
        crate::content::Content::check_required(&args.packs)?;
    }
    let dev = ed_gpu::create_device(ed_gpu::default_backend(), &DeviceDesc { validation: args.validation, frames_in_flight: args.frames_in_flight })
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
        session: None,
        events: Vec::new(),
        ui_pointer: false,
        ui_keyboard: false,
        t_start: Instant::now(),
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
            .with_title("ew (EveryWorld, M4)")
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
                let mut c = c;
                if !self.args.bare {
                    let mut s = Session::new(&mut renderer, &*self.dev, &mut c, self.args.layers.clone(), self.args.mode, true)?;
                    s.inset = self.args.inset;
                    s.collision_debug = self.args.collision_debug;
                    println!(
                        "ew: session: mode {:?}, character {}, {} authored places from {} (Tab/F1 switches mode)",
                        s.mode,
                        s.character_asset,
                        s.authored.places.len(),
                        self.args.layers.display()
                    );
                    self.session = Some(s);
                }
                self.content = Some(c);
            }
        }
        self.gpu = Some(Gpu { renderer: Some(renderer), surface, config, window });
        self.apply_mode();
        Ok(())
    }

    /// Cursor capture for the current mode (game: hidden + locked, editor: free).
    fn apply_mode(&mut self) {
        let (Some(gpu), Some(s)) = (&self.gpu, &self.session) else { return };
        let game = s.mode == Mode::Game;
        if game {
            if gpu.window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                let _ = gpu.window.set_cursor_grab(CursorGrabMode::Confined);
            }
        } else {
            let _ = gpu.window.set_cursor_grab(CursorGrabMode::None);
        }
        gpu.window.set_cursor_visible(!game);
        self.looking = false;
        self.look = Vec2::ZERO;
        self.events.push(InputEvent::PointerGone);
    }

    fn switch_mode(&mut self) {
        if let Some(s) = &mut self.session {
            s.mode = s.mode.toggled();
            println!("ew: {:?} mode", s.mode);
        }
        self.apply_mode();
    }

    /// The M4b frame: session update (fixed steps, editor), then render through the session.
    fn session_frame(&mut self, el: &ActiveEventLoop, frame_dt: f64) {
        let Some(gpu) = &mut self.gpu else { return };
        let (Some(s), Some(content)) = (&mut self.session, &mut self.content) else { return };
        let dev = &*self.dev;
        let renderer = gpu.renderer.as_mut().expect("renderer lives until shutdown");
        let ev = self.hot.poll(renderer, dev, Some(content), false);
        crate::hot::report(&ev);
        s.log_hot(&ev);
        if matches!(ev.shaders, Some(Err(_))) || matches!(ev.packs, Some(Err(_))) {
            self.errors += 1;
        }
        let kb = !self.ui_keyboard;
        let k = |c| if kb && self.keys.contains(&c) { 1.0 } else { 0.0 };
        let cam = CameraInput {
            axes: Vec3::new(k(KeyCode::KeyD) - k(KeyCode::KeyA), k(KeyCode::KeyE) - k(KeyCode::KeyQ), k(KeyCode::KeyW) - k(KeyCode::KeyS)),
            look: Vec2::ZERO,
            fast: kb && (self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight)),
        };
        let size = [gpu.config.width, gpu.config.height];
        let inp = FrameIn {
            dt: frame_dt,
            time: self.t_start.elapsed().as_secs_f64(),
            game: if kb { Self::game_input_of(&self.keys) } else { Default::default() },
            look: std::mem::take(&mut self.look),
            cam,
            events: std::mem::take(&mut self.events),
            size,
            pixels_per_point: gpu.window.scale_factor() as f32,
            camera: None,
        };
        let out = match s.update(renderer, dev, content, inp) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("ew: session: {e}");
                self.errors += 1;
                return;
            }
        };
        self.ui_pointer = out.wants_pointer;
        self.ui_keyboard = out.wants_keyboard;
        if let Err(e) = content.animate(renderer, s.fixed.steps() as f64 / 60.0) {
            eprintln!("ew: {e}");
            self.errors += 1;
        }
        let tf = Instant::now();
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
        let t0 = Instant::now();
        let (w, h) = (gpu.config.width, gpu.config.height);
        let target = ViewTarget { texture: tex, format: gpu.config.format, width: w, height: h, initial: ResourceState::Undefined, final_state: Some(ResourceState::Present) };
        if let Err(e) = s.record(renderer, dev, &mut *enc, target, FrameParams::default(), None) {
            eprintln!("ew: {e}");
            self.errors += 1;
        }
        match dev.end_frame(enc) {
            Ok(()) => self.presented += 1,
            Err(e) => {
                eprintln!("ew: end_frame: {e}");
                self.errors += 1;
            }
        }
        let cpu = flush_ms + t0.elapsed().as_secs_f64() * 1e3;
        self.log_cpu_ms += cpu;
        s.set_cpu_ms(cpu);
        s.after_frame(renderer, dev);
        self.log_frames += 1;
        let since = self.log_t.elapsed().as_secs_f64();
        if since >= 1.0 {
            let t = renderer.timings(dev);
            if let Some(t) = &t {
                s.set_timings(t);
            }
            let (sm, sp) = s.game.as_ref().map(|g| g.step_stats()).map_or((0.0, 0.0), |x| (x.0, x.1));
            println!(
                "ew: {:.1} fps, cpu {:.3} ms/frame, gpu {} ms, {:?} mode, sim step mean {sm:.3} / p99 {sp:.3} ms, {} presented",
                self.log_frames as f64 / since,
                self.log_cpu_ms / self.log_frames.max(1) as f64,
                t.map(|t| format!("{:.3}", t.frame_ms)).unwrap_or_else(|| "-".into()),
                s.mode,
                self.presented
            );
            let _ = renderer.take_cull_history();
            self.log_t = Instant::now();
            self.log_frames = 0;
            self.log_cpu_ms = 0.0;
        }
        if let Some(n) = self.args.frames.filter(|&n| n > 0)
            && self.presented >= n
        {
            println!("ew: {n} frames presented, exiting");
            el.exit();
        }
    }

    fn game_input_of(keys: &BTreeSet<KeyCode>) -> crate::game::GameInput {
        let k = |c| if keys.contains(&c) { 1.0 } else { 0.0 };
        crate::game::GameInput {
            axes: Vec2::new(k(KeyCode::KeyD) - k(KeyCode::KeyA), k(KeyCode::KeyW) - k(KeyCode::KeyS)),
            jump: keys.contains(&KeyCode::Space),
            sprint: keys.contains(&KeyCode::ShiftLeft) || keys.contains(&KeyCode::ShiftRight),
            yaw: 0.0,
        }
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
        if self.session.is_some() {
            self.session_frame(el, frame_dt);
            return;
        }
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
            flags: Default::default(),
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
        if let Some(s) = &mut self.session {
            s.destroy(&*self.dev);
        }
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
        let editor = self.session.as_ref().is_some_and(|s| s.mode == Mode::Editor);
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => self.pending_size = Some(size),
            WindowEvent::ModifiersChanged(m) => {
                if editor {
                    let st = m.state();
                    let mac = cfg!(target_os = "macos");
                    self.events.push(InputEvent::Modifiers(egui::Modifiers {
                        alt: st.alt_key(),
                        ctrl: st.control_key(),
                        shift: st.shift_key(),
                        mac_cmd: mac && st.super_key(),
                        command: if mac { st.super_key() } else { st.control_key() },
                    }));
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else { return };
                let pressed = event.state == ElementState::Pressed;
                if editor {
                    if let Some(key) = egui::Key::from_name(&format!("{code:?}")) {
                        self.events.push(InputEvent::Key { key, pressed, repeat: event.repeat });
                    }
                    if pressed
                        && let Some(t) = event.text.as_ref().filter(|t| !t.chars().any(char::is_control))
                    {
                        self.events.push(InputEvent::Text(t.to_string()));
                    }
                }
                // App keys: only when the editor UI is not typing.
                let free = !(editor && self.ui_keyboard);
                if pressed && !event.repeat {
                    match code {
                        KeyCode::F1 => self.switch_mode(),
                        KeyCode::Tab if free && self.session.is_some() => self.switch_mode(),
                        KeyCode::Escape if free => el.exit(),
                        KeyCode::KeyC if free => {
                            if let Some(s) = &mut self.session {
                                s.collision_debug = !s.collision_debug;
                                println!("ew: collision wireframe {}", if s.collision_debug { "on" } else { "off" });
                            }
                        }
                        KeyCode::KeyI if free => {
                            if let Some(s) = &mut self.session {
                                s.inset = !s.inset;
                                println!("ew: inset view {}", if s.inset { "on" } else { "off" });
                            }
                        }
                        KeyCode::KeyV if free => {
                            if let Some(gpu) = &self.gpu {
                                let size = PhysicalSize::new(gpu.config.width, gpu.config.height);
                                let vsync = !gpu.config.vsync;
                                // Applied now: we are between frames here.
                                self.reconfigure(size, vsync);
                            }
                        }
                        _ => {}
                    }
                }
                if pressed {
                    self.keys.insert(code);
                } else {
                    self.keys.remove(&code);
                }
            }
            WindowEvent::MouseInput { button, state, .. } => {
                let pressed = state == ElementState::Pressed;
                if self.session.is_none() {
                    match button {
                        MouseButton::Left => self.looking = pressed,
                        MouseButton::Right if pressed => self.pick = Some((self.cursor.0.max(0.0) as u32, self.cursor.1.max(0.0) as u32)),
                        _ => {}
                    }
                } else if editor {
                    let b = match button {
                        MouseButton::Left => Some(egui::PointerButton::Primary),
                        MouseButton::Right => Some(egui::PointerButton::Secondary),
                        MouseButton::Middle => Some(egui::PointerButton::Middle),
                        _ => None,
                    };
                    if let Some(button) = b {
                        self.events.push(InputEvent::PointerButton { button, pressed });
                    }
                    // Right-drag flies the camera when the UI does not own the pointer.
                    if button == MouseButton::Right {
                        self.looking = pressed && !self.ui_pointer;
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                if editor {
                    self.events.push(InputEvent::PointerMoved([position.x as f32, position.y as f32]));
                }
            }
            WindowEvent::CursorLeft { .. } if editor => self.events.push(InputEvent::PointerGone),
            WindowEvent::MouseWheel { delta, .. } if editor => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(x, y) => [x, y],
                    MouseScrollDelta::PixelDelta(p) => [p.x as f32 / 24.0, p.y as f32 / 24.0],
                };
                self.events.push(InputEvent::Wheel(d));
            }
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.looking = false;
            }
            WindowEvent::RedrawRequested if !el.exiting() => self.frame(el),
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        let game = self.session.as_ref().is_some_and(|s| s.mode == Mode::Game);
        if let DeviceEvent::MouseMotion { delta } = event
            && (self.looking || game)
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
