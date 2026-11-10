//! Native Metal probe window: triangle + frame stats. `cargo run --release -p ed_metal --bin ed_metal_probe`
#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    probe::run();
}

#[cfg(target_os = "macos")]
mod probe {
    use ed_metal::{timers::GpuTimers, MetalContext};
    use ed_gpu::PassTimes;
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2::{msg_send, MainThreadMarker};
    use objc2_app_kit::NSView;
    use objc2_core_foundation::CGSize;
    use objc2_metal::*;
    use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::ptr::NonNull;
    use std::time::Instant;
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::window::{Window, WindowId};

    struct State {
        window: Window,
        layer: Retained<CAMetalLayer>,
        ctx: MetalContext,
        pso: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
        start: Instant,
        frames: u32,
        last_report: Instant,
        timers: GpuTimers,
        times: PassTimes,
    }

    #[derive(Default)]
    struct App {
        st: Option<State>,
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, el: &ActiveEventLoop) {
            let window = el.create_window(Window::default_attributes().with_title("ed_metal probe")).unwrap();
            let ctx = MetalContext::new().expect("metal");
            println!("device: {}", ctx.name());
            println!("stage-boundary sampling: {}", ctx.device.supportsCounterSampling(MTLCounterSamplingPoint::AtStageBoundary));
            if let Some(sets) = ctx.device.counterSets() {
                for i in 0..sets.count() { println!("counter set: {}", sets.objectAtIndex(i).name()); }
            }
            let lib = ctx.library(include_str!("../../shaders/probe.metal")).expect("msl");
            let vs = lib.newFunctionWithName(&objc2_foundation::NSString::from_str("probe_vs")).unwrap();
            let fs = lib.newFunctionWithName(&objc2_foundation::NSString::from_str("probe_fs")).unwrap();
            let desc = MTLRenderPipelineDescriptor::new();
            desc.setVertexFunction(Some(&vs));
            desc.setFragmentFunction(Some(&fs));
            unsafe { desc.colorAttachments().objectAtIndexedSubscript(0).setPixelFormat(MTLPixelFormat::BGRA8Unorm) };
            let pso = ctx.device.newRenderPipelineStateWithDescriptor_error(&desc).expect("pso");

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
            let _ = MainThreadMarker::new();
            let size = window.inner_size();
            layer.setDrawableSize(CGSize::new(size.width as f64, size.height as f64));
            window.request_redraw();
            let timers = GpuTimers::new(&ctx.device);
            println!("gpu timers supported: {}", timers.supported);
            self.st = Some(State { window, layer, ctx, pso, start: Instant::now(), frames: 0, last_report: Instant::now(), timers, times: PassTimes::new() });
        }

        fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
            let Some(st) = self.st.as_mut() else { return };
            match ev {
                WindowEvent::CloseRequested => el.exit(),
                WindowEvent::Resized(s) => st.layer.setDrawableSize(CGSize::new(s.width as f64, s.height as f64)),
                WindowEvent::RedrawRequested => {
                    st.draw();
                    st.window.request_redraw();
                }
                _ => {}
            }
        }
    }

    impl State {
        fn draw(&mut self) {
            let Some(drawable) = self.layer.nextDrawable() else { return };
            self.timers.begin_frame();
            let cb = self.ctx.queue.commandBuffer().unwrap();
            let pass = MTLRenderPassDescriptor::new();
            let ca = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
            ca.setTexture(Some(&drawable.texture()));
            ca.setLoadAction(MTLLoadAction::Clear);
            ca.setStoreAction(MTLStoreAction::Store);
            ca.setClearColor(MTLClearColor { red: 0.05, green: 0.07, blue: 0.1, alpha: 1.0 });
            self.timers.attach(&pass, "triangle");
            let enc = cb.renderCommandEncoderWithDescriptor(&pass).unwrap();
            enc.setRenderPipelineState(&self.pso);
            let t = self.start.elapsed().as_secs_f32();
            unsafe {
                enc.setVertexBytes_length_atIndex(NonNull::new(&t as *const f32 as *mut _).unwrap(), 4, 0);
                enc.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 3);
            }
            enc.endEncoding();
            cb.presentDrawable(ProtocolObject::from_ref(&*drawable));
            cb.commit();
            self.timers.end_frame();
            for (n, ms) in self.timers.resolve_oldest() { self.times.record(&n, ms); }
            self.frames += 1;
            if self.last_report.elapsed().as_secs_f32() >= 2.0 {
                println!("{:.0} fps | {}", self.frames as f32 / self.last_report.elapsed().as_secs_f32(), self.times.sorted().iter().map(|(n, ms)| format!("{n} {ms:.3} ms")).collect::<Vec<_>>().join(", "));
                self.timers.recalibrate(&self.ctx.device);
                self.frames = 0;
                self.last_report = Instant::now();
            }
        }
    }

    pub fn run() {
        let el = EventLoop::new().unwrap();
        el.set_control_flow(ControlFlow::Poll);
        let mut app = App::default();
        el.run_app(&mut app).unwrap();
    }
}
