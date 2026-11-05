//! Clipmap terrain probe: one instanced draw covers every clipmap block; heights computed in the vertex shader.
//! `cargo run --release -p ed_metal --bin terrain_probe -- [levels] [seconds]`
#[cfg(not(target_os = "macos"))]
fn main() {}
#[cfg(target_os = "macos")]
fn main() {
    probe::run();
}

#[cfg(target_os = "macos")]
mod probe {
    use ed_gpu::clipmap::*;
    use ed_gpu::PassTimes;
    use ed_metal::{timers::GpuTimers, MetalContext};
    use glam::{Mat4, Vec3};
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::NSView;
    use objc2_core_foundation::CGSize;
    use objc2_foundation::NSString;
    use objc2_metal::*;
    use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::ptr::NonNull;
    use std::time::Instant;
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::window::{Window, WindowId};

    type Buf = Retained<ProtocolObject<dyn MTLBuffer>>;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct U { vp: [[f32; 4]; 4], centre: [f32; 2], pad: [f32; 2], sun: [f32; 3], pad2: f32 }

    struct State {
        window: Window,
        layer: Retained<CAMetalLayer>,
        ctx: MetalContext,
        pso: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
        depth_state: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
        depth_tex: Option<(Retained<ProtocolObject<dyn MTLTexture>>, usize, usize)>,
        verts: Buf,
        indices: Buf,
        index_count: usize,
        blocks: Vec<Buf>,
        uni: Vec<Buf>,
        layout: ClipmapLayout,
        start: Instant,
        count: u32,
        last_report: Instant,
        timers: GpuTimers,
        times: PassTimes,
        idx: usize,
        quit_at: f32,
        tris: u64,
    }

    struct App { levels: u32, secs: f32, st: Option<State> }

    fn buf<T>(ctx: &MetalContext, data: &[T]) -> Buf {
        unsafe { ctx.device.newBufferWithBytes_length_options(NonNull::new(data.as_ptr() as *mut _).unwrap(), std::mem::size_of_val(data), MTLResourceOptions::StorageModeShared).unwrap() }
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, el: &ActiveEventLoop) {
            let window = el.create_window(Window::default_attributes().with_title("ed_metal terrain_probe").with_inner_size(winit::dpi::LogicalSize::new(1280, 720))).unwrap();
            let ctx = MetalContext::new().expect("metal");
            let lib = ctx.library(include_str!("../../shaders/terrain_probe.metal")).expect("msl");
            let f = |n: &str| lib.newFunctionWithName(&NSString::from_str(n)).unwrap();
            let layout = ClipmapLayout { levels: self.levels, block: 64, base_spacing_m: 1.0 };
            let mesh = build_block(layout.block);
            let indices: Vec<u16> = mesh.indices.iter().map(|&i| i as u16).collect();
            let d = MTLRenderPipelineDescriptor::new();
            d.setVertexFunction(Some(&f("terrain_vs")));
            d.setFragmentFunction(Some(&f("terrain_fs")));
            d.setDepthAttachmentPixelFormat(MTLPixelFormat::Depth32Float);
            unsafe { d.colorAttachments().objectAtIndexedSubscript(0).setPixelFormat(MTLPixelFormat::BGRA8Unorm) };
            let pso = ctx.device.newRenderPipelineStateWithDescriptor_error(&d).expect("pso");
            let ds = MTLDepthStencilDescriptor::new();
            ds.setDepthCompareFunction(MTLCompareFunction::Less);
            ds.setDepthWriteEnabled(true);
            let depth_state = ctx.device.newDepthStencilStateWithDescriptor(&ds).unwrap();
            let max_blocks = 4 + 12 * self.levels as usize + 16;
            let blocks: Vec<Buf> = (0..3).map(|_| ctx.device.newBufferWithLength_options(max_blocks * 16, MTLResourceOptions::StorageModeShared).unwrap()).collect();
            let uni: Vec<Buf> = (0..3).map(|_| ctx.device.newBufferWithLength_options(std::mem::size_of::<U>(), MTLResourceOptions::StorageModeShared).unwrap()).collect();
            let layer = unsafe { CAMetalLayer::new() };
            layer.setDevice(Some(&ctx.device));
            layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
            unsafe {
                let _: () = msg_send![&layer, setDisplaySyncEnabled: false];
                let _: () = msg_send![&layer, setMaximumDrawableCount: 3usize];
            }
            let RawWindowHandle::AppKit(h) = window.window_handle().unwrap().as_raw() else { panic!("not appkit") };
            let view: &NSView = unsafe { h.ns_view.cast::<NSView>().as_ref() };
            view.setWantsLayer(true);
            view.setLayer(Some(&layer));
            let sz = window.inner_size();
            layer.setDrawableSize(CGSize::new(sz.width as f64, sz.height as f64));
            let verts_b = buf(&ctx, &mesh.vertices);
            let idx_b = buf(&ctx, &indices);
            let timers = GpuTimers::new(&ctx.device);
            let tris = total_triangles(&layout);
            println!("device {} | {} levels | {} tris | coverage {:.0} m | gpu timers {}", ctx.name(), layout.levels, tris, coverage_radius_m(&layout), timers.supported);
            window.request_redraw();
            self.st = Some(State {
                window, layer, ctx, pso, depth_state, depth_tex: None, verts: verts_b, indices: idx_b, index_count: indices.len(), blocks, uni, layout,
                start: Instant::now(), count: 0, last_report: Instant::now(), timers, times: PassTimes::new(), idx: 0, quit_at: self.secs, tris,
            });
        }
        fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
            let Some(st) = self.st.as_mut() else { return };
            match ev {
                WindowEvent::CloseRequested => el.exit(),
                WindowEvent::Resized(s) => st.layer.setDrawableSize(CGSize::new(s.width as f64, s.height as f64)),
                WindowEvent::RedrawRequested => {
                    if st.start.elapsed().as_secs_f32() > st.quit_at { el.exit(); return; }
                    st.draw();
                    st.window.request_redraw();
                }
                _ => {}
            }
        }
    }

    impl State {
        fn depth(&mut self, w: usize, h: usize) -> Retained<ProtocolObject<dyn MTLTexture>> {
            if let Some((t, tw, th)) = &self.depth_tex { if *tw == w && *th == h { return t.clone(); } }
            let d = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(MTLPixelFormat::Depth32Float, w, h, false) };
            d.setUsage(MTLTextureUsage::RenderTarget);
            d.setStorageMode(MTLStorageMode::Private);
            let t = self.ctx.device.newTextureWithDescriptor(&d).unwrap();
            self.depth_tex = Some((t.clone(), w, h));
            t
        }

        fn draw(&mut self) {
            let Some(drawable) = self.layer.nextDrawable() else { return };
            let tex = drawable.texture();
            let (w, h) = (tex.width(), tex.height());
            let depth = self.depth(w, h);
            let t = self.start.elapsed().as_secs_f32();
            // Fly forward over the terrain.
            let pos = Vec3::new(t * 120.0, 1500.0 + 300.0 * (t * 0.3).sin(), t * 60.0);
            let fwd = Vec3::new(0.8, -0.28, 0.4).normalize();
            let view = Mat4::look_at_rh(pos, pos + fwd, Vec3::Y);
            let proj = Mat4::perspective_rh(1.0, w as f32 / h as f32, 2.0, 60000.0);
            let vp = (proj * view).to_cols_array_2d();
            let centre = snapped_centre_m(&self.layout, [pos.x as f64, pos.z as f64]);
            let inst = build_level_instances(&self.layout, [pos.x as f64, pos.z as f64]);
            let slot = self.idx % 3;
            let data: Vec<[f32; 4]> = inst.iter().map(|b| [b.offset_m[0], b.offset_m[1], b.scale_m, b.level as f32]).collect();
            unsafe {
                std::ptr::copy_nonoverlapping(data.as_ptr() as *const u8, self.blocks[slot].contents().as_ptr() as *mut u8, data.len() * 16);
                let u = U { vp, centre: [centre[0] as f32, centre[1] as f32], pad: [0.0; 2], sun: Vec3::new(0.5, 0.7, 0.3).normalize().to_array(), pad2: 0.0 };
                std::ptr::copy_nonoverlapping(&u as *const U as *const u8, self.uni[slot].contents().as_ptr() as *mut u8, std::mem::size_of::<U>());
            }
            self.timers.begin_frame();
            let cb = self.ctx.queue.commandBuffer().unwrap();
            let pass = MTLRenderPassDescriptor::new();
            let ca = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
            ca.setTexture(Some(&tex));
            ca.setLoadAction(MTLLoadAction::Clear);
            ca.setStoreAction(MTLStoreAction::Store);
            ca.setClearColor(MTLClearColor { red: 0.55, green: 0.7, blue: 0.9, alpha: 1.0 });
            let da = pass.depthAttachment();
            da.setTexture(Some(&depth));
            da.setLoadAction(MTLLoadAction::Clear);
            da.setStoreAction(MTLStoreAction::DontCare);
            da.setClearDepth(1.0);
            self.timers.attach(&pass, "terrain");
            let re = cb.renderCommandEncoderWithDescriptor(&pass).unwrap();
            re.setRenderPipelineState(&self.pso);
            re.setDepthStencilState(Some(&self.depth_state));
            re.setCullMode(MTLCullMode::Back);
            unsafe {
                re.setVertexBuffer_offset_atIndex(Some(&self.verts), 0, 0);
                re.setVertexBuffer_offset_atIndex(Some(&self.blocks[slot]), 0, 1);
                re.setVertexBuffer_offset_atIndex(Some(&self.uni[slot]), 0, 2);
                re.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(MTLPrimitiveType::Triangle, self.index_count, MTLIndexType::UInt16, &self.indices, 0, inst.len());
            }
            re.endEncoding();
            cb.presentDrawable(ProtocolObject::from_ref(&*drawable));
            cb.commit();
            self.timers.end_frame();
            for (n, ms) in self.timers.resolve_oldest() { self.times.record(&n, ms); }
            self.idx += 1;
            self.count += 1;
            if self.last_report.elapsed().as_secs_f32() >= 2.0 {
                println!("{:.0} fps | {} | {} tris", self.count as f32 / self.last_report.elapsed().as_secs_f32(),
                    self.times.sorted().iter().map(|(n, ms)| format!("{n} {ms:.3} ms")).collect::<Vec<_>>().join(", "), self.tris);
                self.count = 0;
                self.last_report = Instant::now();
                self.timers.recalibrate(&self.ctx.device);
            }
        }
    }

    pub fn run() {
        let a: Vec<String> = std::env::args().collect();
        let levels = a.get(1).and_then(|v| v.parse().ok()).unwrap_or(8u32);
        let secs = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(10.0f32);
        let el = EventLoop::new().unwrap();
        el.set_control_flow(ControlFlow::Poll);
        let mut app = App { levels, secs, st: None };
        el.run_app(&mut app).unwrap();
    }
}
