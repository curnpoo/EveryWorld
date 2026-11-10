//! GPU-driven rendering probe: N instances culled by a compute kernel that encodes draws into an
//! indirect command buffer; the CPU issues one executeCommands per frame.
//! `cargo run --release -p ed_metal --bin icb_probe -- [instances] [seconds]`
#[cfg(not(target_os = "macos"))]
fn main() {}
#[cfg(target_os = "macos")]
fn main() {
    probe::run();
}

#[cfg(target_os = "macos")]
mod probe {
    use ed_gpu::cull::{frustum_planes, Instance};
    use ed_gpu::PassTimes;
    use ed_metal::{timers::GpuTimers, MetalContext};
    use glam::{Mat4, Vec3};
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2::msg_send;
    use objc2_app_kit::NSView;
    use objc2_core_foundation::CGSize;
    use objc2_foundation::{NSRange, NSString};
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
    struct Frame {
        planes: [[f32; 4]; 6],
        vp: [[f32; 4]; 4],
        count: u32,
        max_draws: u32,
        pad: [u32; 2],
    }

    struct State {
        window: Window,
        layer: Retained<CAMetalLayer>,
        ctx: MetalContext,
        cull_pso: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
        pso: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
        depth_state: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
        depth_tex: Option<(Retained<ProtocolObject<dyn MTLTexture>>, usize, usize)>,
        icb: Retained<ProtocolObject<dyn MTLIndirectCommandBuffer>>,
        container: Buf,
        counter: Buf,
        insts: Buf,
        verts: Buf,
        indices: Buf,
        frames: Vec<Buf>,
        n: u32,
        start: Instant,
        count: u32,
        last_report: Instant,
        timers: GpuTimers,
        times: PassTimes,
        idx: usize,
        quit_at: f32,
    }

    struct App {
        n: u32,
        secs: f32,
        st: Option<State>,
    }

    fn buf<T>(ctx: &MetalContext, data: &[T]) -> Buf {
        unsafe {
            ctx.device
                .newBufferWithBytes_length_options(NonNull::new(data.as_ptr() as *mut _).unwrap(), std::mem::size_of_val(data), MTLResourceOptions::StorageModeShared)
                .unwrap()
        }
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, el: &ActiveEventLoop) {
            let window = el.create_window(Window::default_attributes().with_title("ed_metal icb_probe").with_inner_size(winit::dpi::LogicalSize::new(1280, 720))).unwrap();
            let ctx = MetalContext::new().expect("metal");
            let lib = ctx.library(include_str!("../../shaders/icb_probe.metal")).expect("msl");
            let f = |n: &str| lib.newFunctionWithName(&NSString::from_str(n)).unwrap();

            // Cube mesh.
            let mut verts: Vec<[f32; 3]> = vec![];
            for z in [-1.0f32, 1.0] { for y in [-1.0f32, 1.0] { for x in [-1.0f32, 1.0] { verts.push([x, y, z]); } } }
            let indices: Vec<u16> = vec![0, 2, 1, 1, 2, 3, 4, 5, 6, 5, 7, 6, 0, 1, 4, 1, 5, 4, 2, 6, 3, 3, 6, 7, 0, 4, 2, 2, 4, 6, 1, 3, 5, 3, 7, 5];
            // Instances on a grid, deterministic pseudo-random heights.
            let side = (self.n as f32).sqrt().ceil() as u32;
            let mut rng = 0x1234_5678u32;
            let mut r = || { rng = rng.wrapping_mul(1664525).wrapping_add(1013904223); (rng >> 8) as f32 / 16777216.0 };
            let insts: Vec<Instance> = (0..self.n)
                .map(|i| {
                    let (gx, gz) = ((i % side) as f32, (i / side) as f32);
                    let h = r() * 60.0;
                    Instance { center: [gx * 12.0 - side as f32 * 6.0, h, gz * 12.0 - side as f32 * 6.0], radius: 2.0 + r() * 3.0, mesh: 0, first_index: 0, index_count: 36, base_vertex: 0 }
                })
                .collect();
            let insts_b = buf(&ctx, &insts);
            let verts_b = buf(&ctx, &verts);
            let idx_b = buf(&ctx, &indices);
            let counter = ctx.device.newBufferWithLength_options(4, MTLResourceOptions::StorageModePrivate).unwrap();

            let cull_pso = ctx.device.newComputePipelineStateWithFunction_error(&f("cull_encode")).expect("cull pso");
            let d = MTLRenderPipelineDescriptor::new();
            d.setVertexFunction(Some(&f("icb_vs")));
            d.setFragmentFunction(Some(&f("icb_fs")));
            d.setSupportIndirectCommandBuffers(true);
            d.setDepthAttachmentPixelFormat(MTLPixelFormat::Depth32Float);
            unsafe { d.colorAttachments().objectAtIndexedSubscript(0).setPixelFormat(MTLPixelFormat::BGRA8Unorm) };
            let pso = ctx.device.newRenderPipelineStateWithDescriptor_error(&d).expect("pso");
            let ds = MTLDepthStencilDescriptor::new();
            ds.setDepthCompareFunction(MTLCompareFunction::Less);
            ds.setDepthWriteEnabled(true);
            let depth_state = ctx.device.newDepthStencilStateWithDescriptor(&ds).unwrap();

            // ICB: one slot per instance (worst case); only visible ones are encoded.
            let id = MTLIndirectCommandBufferDescriptor::new();
            id.setCommandTypes(MTLIndirectCommandType::DrawIndexed);
            id.setInheritBuffers(false);
            id.setInheritPipelineState(true);
            id.setMaxVertexBufferBindCount(3);
            let icb = unsafe { ctx.device.newIndirectCommandBufferWithDescriptor_maxCommandCount_options(&id, self.n as usize, MTLResourceOptions::StorageModePrivate) }.expect("icb");
            let enc = unsafe { f("cull_encode").newArgumentEncoderWithBufferIndex(5) };
            let container = ctx.device.newBufferWithLength_options(enc.encodedLength(), MTLResourceOptions::StorageModeShared).unwrap();
            unsafe { enc.setArgumentBuffer_offset(Some(&container), 0) };
            unsafe { enc.setIndirectCommandBuffer_atIndex(Some(&icb), 0) };

            let frames: Vec<Buf> = (0..3).map(|_| ctx.device.newBufferWithLength_options(std::mem::size_of::<Frame>(), MTLResourceOptions::StorageModeShared).unwrap()).collect();

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
            let timers = GpuTimers::new(&ctx.device);
            println!("device {} | {} instances | gpu timers {}", ctx.name(), self.n, timers.supported);
            window.request_redraw();
            self.st = Some(State {
                window, layer, ctx, cull_pso, pso, depth_state, depth_tex: None, icb, container, counter, insts: insts_b, verts: verts_b, indices: idx_b, frames, n: self.n,
                start: Instant::now(), count: 0, last_report: Instant::now(), timers, times: PassTimes::new(), idx: 0, quit_at: self.secs,
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
            let side = (self.n as f32).sqrt() * 12.0;
            let ang = t * 0.15;
            let eye = Vec3::new(ang.cos() * side * 0.25, 220.0, ang.sin() * side * 0.25);
            let view = Mat4::look_at_rh(eye, Vec3::new(ang.cos() * side * 0.25 + (ang + 1.0).cos() * 400.0, 0.0, ang.sin() * side * 0.25 + (ang + 1.0).sin() * 400.0), Vec3::Y);
            let proj = Mat4::perspective_rh(1.0, w as f32 / h as f32, 1.0, 6000.0);
            let vp = proj * view;
            let vpa = vp.to_cols_array_2d();
            let frame = Frame { planes: frustum_planes(vpa), vp: vpa, count: self.n, max_draws: self.n, pad: [0; 2] };
            let fb = &self.frames[self.idx % 3];
            unsafe { std::ptr::copy_nonoverlapping(&frame as *const Frame as *const u8, fb.contents().as_ptr() as *mut u8, std::mem::size_of::<Frame>()) };

            self.timers.begin_frame();
            let cb = self.ctx.queue.commandBuffer().unwrap();

            // 1. Reset ICB + counter.
            let bp = MTLBlitPassDescriptor::new();
            self.timers.attach_blit(&bp, "reset");
            let blit = cb.blitCommandEncoderWithDescriptor(&bp).unwrap();
            unsafe { blit.resetCommandsInBuffer_withRange(&self.icb, NSRange { location: 0, length: self.n as usize }) };
            blit.fillBuffer_range_value(&self.counter, NSRange { location: 0, length: 4 }, 0);
            blit.endEncoding();

            // 2. Cull + encode.
            let cp = MTLComputePassDescriptor::new();
            self.timers.attach_compute(&cp, "cull_encode");
            let ce = cb.computeCommandEncoderWithDescriptor(&cp).unwrap();
            ce.setComputePipelineState(&self.cull_pso);
            unsafe {
                ce.setBuffer_offset_atIndex(Some(fb), 0, 0);
                ce.setBuffer_offset_atIndex(Some(&self.insts), 0, 1);
                ce.setBuffer_offset_atIndex(Some(&self.counter), 0, 2);
                ce.setBuffer_offset_atIndex(Some(&self.indices), 0, 3);
                ce.setBuffer_offset_atIndex(Some(&self.verts), 0, 4);
                ce.setBuffer_offset_atIndex(Some(&self.container), 0, 5);
            }
            ce.useResource_usage(ProtocolObject::from_ref(&*self.icb), MTLResourceUsage::Write);
            ce.dispatchThreads_threadsPerThreadgroup(MTLSize { width: self.n as usize, height: 1, depth: 1 }, MTLSize { width: 64, height: 1, depth: 1 });
            ce.endEncoding();

            // 3. Draw everything the kernel encoded with one call.
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
            self.timers.attach(&pass, "main_icb");
            let re = cb.renderCommandEncoderWithDescriptor(&pass).unwrap();
            re.setRenderPipelineState(&self.pso);
            re.setDepthStencilState(Some(&self.depth_state));
            re.setCullMode(MTLCullMode::Back);
            for b in [&self.insts, &self.verts, &self.indices, fb] {
                re.useResource_usage_stages(ProtocolObject::from_ref(&**b), MTLResourceUsage::Read, MTLRenderStages::Vertex);
            }
            unsafe { re.executeCommandsInBuffer_withRange(&self.icb, NSRange { location: 0, length: self.n as usize }) };
            re.endEncoding();
            cb.presentDrawable(ProtocolObject::from_ref(&*drawable));
            cb.commit();
            self.timers.end_frame();
            for (n, ms) in self.timers.resolve_oldest() { self.times.record(&n, ms); }
            self.idx += 1;
            self.count += 1;
            if self.last_report.elapsed().as_secs_f32() >= 2.0 {
                println!("{:.0} fps | {} | total {:.2} ms", self.count as f32 / self.last_report.elapsed().as_secs_f32(),
                    self.times.sorted().iter().map(|(n, ms)| format!("{n} {ms:.3}")).collect::<Vec<_>>().join(", "), self.times.total_avg());
                self.count = 0;
                self.last_report = Instant::now();
                self.timers.recalibrate(&self.ctx.device);
            }
        }
    }

    pub fn run() {
        let a: Vec<String> = std::env::args().collect();
        let n = a.get(1).and_then(|v| v.parse().ok()).unwrap_or(1_000_000u32);
        let secs = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(12.0f32);
        let el = EventLoop::new().unwrap();
        el.set_control_flow(ControlFlow::Poll);
        let mut app = App { n, secs, st: None };
        el.run_app(&mut app).unwrap();
    }
}
