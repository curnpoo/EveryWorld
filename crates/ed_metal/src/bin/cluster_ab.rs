//! Isolated A/B for cluster rendering on one real building (headless, no window).
//!
//! Build the input once: `cargo run --release -p ed_world --example dump_building -- 46.0207 7.7491 building.bin`
//! Then: `cargo run --release -p ed_metal --bin cluster_ab -- building.bin [frames] [out_dir]`
//!
//! A = plain: CPU frustum cull, one drawIndexed per visible instance over the whole building
//!     (the shape of the native path today).
//! B = clusters: `ed_gpu::cluster` clusters, GPU cull over (instance, cluster) pairs (frustum, and
//!     backface cone when `cone` is on), then one mesh-shader threadgroup per visible pair.
//!
//! Checks: GPU visible pairs and triangles against the CPU reference (`ed_gpu::cluster`), pixel
//! diff of the images, and GPU time per frame (command buffer GPU start/end) over instance counts.

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    ab::run();
}

#[cfg(target_os = "macos")]
mod ab {
    use ed_gpu::cluster::{self, Cluster};
    use ed_gpu::cull::{frustum_planes, sphere_visible};
    use ed_metal::MetalContext;
    use glam::{Mat4, Vec3};
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2_foundation::NSString;
    use objc2_metal::*;
    use std::ffi::c_void;
    use std::fmt::Write as _;
    use std::path::PathBuf;
    use std::ptr::NonNull;
    use std::time::Instant;

    type Buf = Retained<ProtocolObject<dyn MTLBuffer>>;
    type Tex = Retained<ProtocolObject<dyn MTLTexture>>;

    const W: usize = 1280;
    const H: usize = 720;
    const SIZES: [usize; 4] = [1, 100, 400, 1600];

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Frame {
        planes: [[f32; 4]; 6],
        vp: [[f32; 4]; 4],
        cam: [f32; 4],
        instances: u32,
        clusters: u32,
        max_pairs: u32,
        pad: u32,
        threshold_px: f32,
        screen_height: f32,
        fov_y: f32,
        slope: f32,
        pad2: u32,
    }

    struct Building {
        pos: Vec<[f32; 3]>,
        nrm: Vec<[f32; 3]>,
        col: Vec<[f32; 4]>,
        idx: Vec<u32>,
        center: [f32; 3],
        radius: f32,
    }

    fn load(path: &str) -> Building {
        let b = std::fs::read(path).unwrap_or_else(|e| panic!("read {path}: {e} (run the dump_building example first)"));
        assert_eq!(&b[0..4], b"EWB1", "not a building dump");
        let nv = u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
        let ni = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
        let mut at = 12;
        let mut floats = |n: usize| -> Vec<f32> {
            let v = b[at..at + 4 * n].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
            at += 4 * n;
            v
        };
        let p = floats(3 * nv);
        let n = floats(3 * nv);
        let c = floats(4 * nv);
        let idx: Vec<u32> = b[at..at + 4 * ni].chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
        let pos: Vec<[f32; 3]> = p.chunks_exact(3).map(|v| [v[0], v[1], v[2]]).collect();
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for q in &pos {
            for k in 0..3 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        let center = [0, 1, 2].map(|k| (lo[k] + hi[k]) * 0.5);
        let radius = (0..3).map(|k| (hi[k] - lo[k]).powi(2)).sum::<f32>().sqrt() * 0.5;
        Building {
            pos,
            nrm: n.chunks_exact(3).map(|v| [v[0], v[1], v[2]]).collect(),
            col: c.chunks_exact(4).map(|v| [v[0], v[1], v[2], v[3]]).collect(),
            idx,
            center,
            radius,
        }
    }

    fn buf<T>(ctx: &MetalContext, data: &[T]) -> Buf {
        unsafe {
            ctx.device
                .newBufferWithBytes_length_options(NonNull::new(data.as_ptr() as *mut _).unwrap(), std::mem::size_of_val(data), MTLResourceOptions::StorageModeShared)
                .unwrap()
        }
    }

    fn empty(ctx: &MetalContext, bytes: usize) -> Buf {
        ctx.device.newBufferWithLength_options(bytes.max(16), MTLResourceOptions::StorageModeShared).unwrap()
    }

    fn write_u32(b: &Buf, v: u32) {
        unsafe { *(b.contents().as_ptr() as *mut u32) = v };
    }

    fn read_u32(b: &Buf) -> u32 {
        unsafe { *(b.contents().as_ptr() as *const u32) }
    }

    /// One scene: a set of building instances (translation only), as offsets.
    struct Scene {
        offsets: Vec<[f32; 4]>,
        inst: Buf,
        pairs: Buf,
        indirect: Buf,
        counter: Buf,
        tris: Buf,
    }

    struct Rig {
        ctx: MetalContext,
        cull: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
        finish: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
        mesh: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
        base: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
        ds: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
        color: Tex,
        depth: Tex,
        frame: Buf,
        // path A
        a_pos: Buf,
        a_nrm: Buf,
        a_col: Buf,
        a_idx: Buf,
        a_idx_rev: Buf,
        // path B
        clusters: Vec<Cluster>,
        c_buf: Buf,
        c_pos: Buf,
        c_nrm: Buf,
        c_col: Buf,
        c_lidx: Buf,
        b: Building,
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Path {
        Plain,
        PlainRev,
        Clusters { cone: bool },
    }

    impl Path {
        fn label(self) -> &'static str {
            match self {
                Path::Plain => "A plain",
                Path::PlainRev => "A reversed (control)",
                Path::Clusters { cone: false } => "B clusters (no cone)",
                Path::Clusters { cone: true } => "B clusters (cone)",
            }
        }
    }

    struct Shot {
        gpu_ms: f64,
        cpu_ms: f64,
        draws: u32,
        pairs: u32,
        tris: u32,
        image: Vec<u8>,
    }

    fn new_rig(b: Building) -> Rig {
        let ctx = MetalContext::new().expect("metal");
        let lib = ctx.library(include_str!("../../shaders/cluster_ab.metal")).expect("msl");
        let f = |n: &str| lib.newFunctionWithName(&NSString::from_str(n)).unwrap();
        let cull = ctx.device.newComputePipelineStateWithFunction_error(&f("cluster_cull")).expect("cull pso");
        let finish = ctx.device.newComputePipelineStateWithFunction_error(&f("cluster_finish")).expect("finish pso");

        let md = MTLMeshRenderPipelineDescriptor::new();
        unsafe {
            md.setMeshFunction(Some(&f("cluster_mesh")));
            md.setFragmentFunction(Some(&f("shade_fs")));
        }
        md.setMaxTotalThreadsPerMeshThreadgroup(128);
        md.setDepthAttachmentPixelFormat(MTLPixelFormat::Depth32Float);
        unsafe { md.colorAttachments().objectAtIndexedSubscript(0).setPixelFormat(MTLPixelFormat::BGRA8Unorm) };
        let mesh = ctx
            .device
            .newRenderPipelineStateWithMeshDescriptor_options_reflection_error(&md, MTLPipelineOption::None, None)
            .expect("mesh pso");

        let rd = MTLRenderPipelineDescriptor::new();
        rd.setVertexFunction(Some(&f("base_vs")));
        rd.setFragmentFunction(Some(&f("shade_fs")));
        rd.setDepthAttachmentPixelFormat(MTLPixelFormat::Depth32Float);
        unsafe { rd.colorAttachments().objectAtIndexedSubscript(0).setPixelFormat(MTLPixelFormat::BGRA8Unorm) };
        let base = ctx.device.newRenderPipelineStateWithDescriptor_error(&rd).expect("base pso");

        let ds_desc = MTLDepthStencilDescriptor::new();
        ds_desc.setDepthCompareFunction(MTLCompareFunction::Less);
        ds_desc.setDepthWriteEnabled(true);
        let ds = ctx.device.newDepthStencilStateWithDescriptor(&ds_desc).unwrap();

        let hier = cluster::build_hierarchy(&b.pos, &b.idx);
        let cm = cluster::flatten(&hier);
        let clusters = cm.clusters.clone();
        let c_pos: Vec<[f32; 3]> = cm.vertices.clone();
        let c_nrm: Vec<[f32; 3]> = cm.src.iter().map(|&s| b.nrm[s as usize]).collect();
        let c_col: Vec<[f32; 4]> = cm.src.iter().map(|&s| b.col[s as usize]).collect();

        let rev: Vec<u32> = b.idx.chunks_exact(3).rev().flat_map(|t| [t[0], t[1], t[2]]).collect();
        let rig = Rig {
            a_idx_rev: buf(&ctx, &rev),
            cull,
            finish,
            mesh,
            base,
            ds,
            color: target(&ctx, MTLPixelFormat::BGRA8Unorm, MTLStorageMode::Shared, MTLTextureUsage::RenderTarget),
            depth: target(&ctx, MTLPixelFormat::Depth32Float, MTLStorageMode::Private, MTLTextureUsage::RenderTarget),
            frame: empty(&ctx, std::mem::size_of::<Frame>()),
            a_pos: buf(&ctx, &b.pos),
            a_nrm: buf(&ctx, &b.nrm),
            a_col: buf(&ctx, &b.col),
            a_idx: buf(&ctx, &b.idx),
            c_buf: buf(&ctx, &clusters),
            c_pos: buf(&ctx, &c_pos),
            c_nrm: buf(&ctx, &c_nrm),
            c_col: buf(&ctx, &c_col),
            c_lidx: buf(&ctx, &cm.indices),
            clusters,
            b,
            ctx,
        };
        rig
    }

    fn target(ctx: &MetalContext, fmt: MTLPixelFormat, storage: MTLStorageMode, usage: MTLTextureUsage) -> Tex {
        let d = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(fmt, W, H, false) };
        d.setUsage(usage);
        d.setStorageMode(storage);
        ctx.device.newTextureWithDescriptor(&d).unwrap()
    }

    fn scene(rig: &Rig, offsets: Vec<[f32; 4]>) -> Scene {
        let total = offsets.len() * rig.clusters.len();
        Scene {
            inst: buf(&rig.ctx, &offsets),
            pairs: empty(&rig.ctx, total * 8),
            indirect: empty(&rig.ctx, 12),
            counter: empty(&rig.ctx, 4),
            tris: empty(&rig.ctx, 4),
            offsets,
        }
    }

    fn grid(n: usize, spacing: f32) -> Vec<[f32; 4]> {
        let side = (n as f32).sqrt().ceil() as usize;
        let half = (side as f32 - 1.0) * spacing * 0.5;
        (0..side * side)
            .map(|i| [(i % side) as f32 * spacing - half, 0.0, (i / side) as f32 * spacing - half, 0.0])
            .collect()
    }

    fn view(eye: Vec3, target: Vec3) -> Mat4 {
        let proj = Mat4::perspective_rh(50f32.to_radians(), W as f32 / H as f32, 1.0, 8000.0);
        proj * Mat4::look_at_rh(eye, target, Vec3::Y)
    }

    /// CPU reference for the GPU cull: the same (instance, cluster) pairs, counted and summed.
    fn reference(clusters: &[Cluster], offsets: &[[f32; 4]], planes: &[[f32; 4]; 6], cam: Vec3, cone: bool, thr: f32, sh: f32, fov: f32) -> (u32, u32) {
        let (mut pairs, mut tris) = (0u32, 0u32);
        for o in offsets {
            for c in clusters {
                let mut cc = *c;
                cc.center = [c.center[0] + o[0], c.center[1] + o[1], c.center[2] + o[2]];
                let vis = if cone { cluster::cluster_visible(&cc, planes, cam.into()) } else { sphere_visible(planes, cc.center, cc.radius) };
                if !vis { continue; }
                // LOD selection
                let dx = cc.center[0] - cam.x;
                let dy = cc.center[1] - cam.y;
                let dz = cc.center[2] - cam.z;
                let dist = (dx*dx+dy*dy+dz*dz).sqrt();
                let slope = thr * 2.0 * (fov * 0.5).tan() / sh;
                if dist <= 0.0 { continue; }
                if !(cc.error <= slope * dist && cc.parent_error > slope * dist) { continue; }
                pairs += 1;
                tris += c.tri_count;
            }
        }
        (pairs, tris)
    }

    impl Rig {
        fn draw(&self, path: Path, sc: &Scene, vp: Mat4, cam: Vec3, want_image: bool, thr: f32) -> Shot {
            let planes = frustum_planes(vp.to_cols_array_2d());
            let cone = matches!(path, Path::Clusters { cone: true });
            let frame = Frame {
                planes,
                vp: vp.to_cols_array_2d(),
                cam: [cam.x, cam.y, cam.z, if cone { 1.0 } else { 0.0 }],
                instances: sc.offsets.len() as u32,
                clusters: self.clusters.len() as u32,
                max_pairs: (sc.offsets.len() * self.clusters.len()) as u32,
                pad: 0,
                threshold_px: thr,
                screen_height: H as f32,
                fov_y: 50.0f32.to_radians(),
                slope: thr * 2.0 * (50.0f32.to_radians() * 0.5).tan() / (H as f32),
                pad2: 0,
            };
            unsafe { std::ptr::write(self.frame.contents().as_ptr() as *mut Frame, frame) };
            let cpu0 = Instant::now();
            let rp = MTLRenderPassDescriptor::new();
            let ca = unsafe { rp.colorAttachments().objectAtIndexedSubscript(0) };
            ca.setTexture(Some(&self.color));
            ca.setLoadAction(MTLLoadAction::Clear);
            ca.setStoreAction(MTLStoreAction::Store);
            ca.setClearColor(MTLClearColor { red: 0.55, green: 0.7, blue: 0.9, alpha: 1.0 });
            let da = rp.depthAttachment();
            da.setTexture(Some(&self.depth));
            da.setLoadAction(MTLLoadAction::Clear);
            da.setStoreAction(MTLStoreAction::DontCare);
            da.setClearDepth(1.0);

            let cb = self.ctx.queue.commandBuffer().unwrap();
            let (draws, pairs, tris);
            match path {
                Path::Plain | Path::PlainRev => {
                    let ib = if path == Path::PlainRev { &self.a_idx_rev } else { &self.a_idx };
                    let vis: Vec<[f32; 4]> = sc
                        .offsets
                        .iter()
                        .copied()
                        .filter(|o| sphere_visible(&planes, [self.b.center[0] + o[0], self.b.center[1] + o[1], self.b.center[2] + o[2]], self.b.radius))
                        .collect();
                    draws = vis.len() as u32;
                    pairs = 0;
                    tris = vis.len() as u32 * self.b.idx.len() as u32 / 3;
                    let re = cb.renderCommandEncoderWithDescriptor(&rp).unwrap();
                    re.setDepthStencilState(Some(&self.ds));
                    re.setCullMode(MTLCullMode::None);
                    re.setRenderPipelineState(&self.base);
                    unsafe {
                        re.setVertexBuffer_offset_atIndex(Some(&self.a_pos), 0, 0);
                        re.setVertexBuffer_offset_atIndex(Some(&self.a_nrm), 0, 1);
                        re.setVertexBuffer_offset_atIndex(Some(&self.a_col), 0, 2);
                        re.setVertexBuffer_offset_atIndex(Some(&self.frame), 0, 4);
                    }
                    for o in &vis {
                        unsafe {
                            re.setVertexBytes_length_atIndex(NonNull::new(o as *const _ as *mut c_void).unwrap(), 16, 3);
                            re.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(MTLPrimitiveType::Triangle, self.b.idx.len(), MTLIndexType::UInt32, ib, 0);
                        }
                    }
                    re.endEncoding();
                }
                Path::Clusters { .. } => {
                    write_u32(&sc.counter, 0);
                    write_u32(&sc.tris, 0);
                    let total = sc.offsets.len() * self.clusters.len();
                    let ce = cb.computeCommandEncoder().unwrap();
                    ce.setComputePipelineState(&self.cull);
                    unsafe {
                        ce.setBuffer_offset_atIndex(Some(&self.frame), 0, 0);
                        ce.setBuffer_offset_atIndex(Some(&self.c_buf), 0, 1);
                        ce.setBuffer_offset_atIndex(Some(&sc.inst), 0, 2);
                        ce.setBuffer_offset_atIndex(Some(&sc.pairs), 0, 3);
                        ce.setBuffer_offset_atIndex(Some(&sc.counter), 0, 4);
                        ce.setBuffer_offset_atIndex(Some(&sc.tris), 0, 5);
                    }
                    ce.dispatchThreads_threadsPerThreadgroup(MTLSize { width: total, height: 1, depth: 1 }, MTLSize { width: 64, height: 1, depth: 1 });
                    ce.setComputePipelineState(&self.finish);
                    unsafe {
                        ce.setBuffer_offset_atIndex(Some(&self.frame), 0, 0);
                        ce.setBuffer_offset_atIndex(Some(&sc.counter), 0, 1);
                        ce.setBuffer_offset_atIndex(Some(&sc.indirect), 0, 2);
                    }
                    ce.dispatchThreads_threadsPerThreadgroup(MTLSize { width: 1, height: 1, depth: 1 }, MTLSize { width: 1, height: 1, depth: 1 });
                    ce.endEncoding();

                    let re = cb.renderCommandEncoderWithDescriptor(&rp).unwrap();
                    re.setDepthStencilState(Some(&self.ds));
                    re.setCullMode(MTLCullMode::None);
                    re.setRenderPipelineState(&self.mesh);
                    unsafe {
                        re.setMeshBuffer_offset_atIndex(Some(&self.frame), 0, 0);
                        re.setMeshBuffer_offset_atIndex(Some(&sc.pairs), 0, 1);
                        re.setMeshBuffer_offset_atIndex(Some(&sc.inst), 0, 2);
                        re.setMeshBuffer_offset_atIndex(Some(&self.c_buf), 0, 3);
                        re.setMeshBuffer_offset_atIndex(Some(&self.c_pos), 0, 4);
                        re.setMeshBuffer_offset_atIndex(Some(&self.c_nrm), 0, 5);
                        re.setMeshBuffer_offset_atIndex(Some(&self.c_col), 0, 6);
                        re.setMeshBuffer_offset_atIndex(Some(&self.c_lidx), 0, 7);
                        re.setMeshBuffer_offset_atIndex(Some(&sc.counter), 0, 8);
                        re.drawMeshThreadgroupsWithIndirectBuffer_indirectBufferOffset_threadsPerObjectThreadgroup_threadsPerMeshThreadgroup(
                            &sc.indirect,
                            0,
                            MTLSize { width: 1, height: 1, depth: 1 },
                            MTLSize { width: 128, height: 1, depth: 1 },
                        );
                    }
                    re.endEncoding();
                    draws = 1;
                    pairs = 0;
                    tris = 0;
                }
            }
            let cpu_ms = cpu0.elapsed().as_secs_f64() * 1000.0;
            cb.commit();
            cb.waitUntilCompleted();
            let gpu_ms = (cb.GPUEndTime() - cb.GPUStartTime()) * 1000.0;
            let (pairs, tris) = match path {
                Path::Plain | Path::PlainRev => (pairs, tris),
                Path::Clusters { .. } => (read_u32(&sc.counter).min((sc.offsets.len() * self.clusters.len()) as u32), read_u32(&sc.tris)),
            };
            let image = if want_image { self.readback() } else { Vec::new() };
            Shot { gpu_ms, cpu_ms, draws, pairs, tris, image }
        }

        fn readback(&self) -> Vec<u8> {
            let mut out = vec![0u8; W * H * 4];
            let region = MTLRegion { origin: MTLOrigin { x: 0, y: 0, z: 0 }, size: MTLSize { width: W, height: H, depth: 1 } };
            unsafe {
                self.color.getBytes_bytesPerRow_fromRegion_mipmapLevel(NonNull::new(out.as_mut_ptr() as *mut c_void).unwrap(), W * 4, region, 0);
            }
            out
        }
    }

    fn diff(a: &[u8], b: &[u8]) -> (f64, f64, u8) {
        let (mut sum, mut big, mut max) = (0f64, 0usize, 0u8);
        for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
            let mut d_px = 0u8;
            for k in 0..3 {
                let d = pa[k].abs_diff(pb[k]);
                sum += d as f64;
                d_px = d_px.max(d);
            }
            max = max.max(d_px);
            if d_px > 8 {
                big += 1;
            }
        }
        let n = (a.len() / 4) as f64;
        (sum / (n * 3.0), big as f64 / n * 100.0, max)
    }

    fn write_ppm(path: &PathBuf, bgra: &[u8]) {
        let mut out = format!("P6\n{W} {H}\n255\n").into_bytes();
        for px in bgra.chunks_exact(4) {
            out.extend_from_slice(&[px[2], px[1], px[0]]);
        }
        std::fs::write(path, out).expect("write ppm");
    }

    fn median(v: &mut [f64]) -> f64 {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    }

    pub fn run() {
        let args: Vec<String> = std::env::args().collect();
        let input = args.get(1).cloned().unwrap_or_else(|| "building.bin".into());
        let frames: usize = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(120);
        let out_dir = PathBuf::from(args.get(3).cloned().unwrap_or_else(|| "cluster_ab_out".into()));
        std::fs::create_dir_all(&out_dir).unwrap();
        let mut rep = String::new();
        macro_rules! say {
            ($($t:tt)*) => {{ let s = format!($($t)*); println!("{s}"); let _ = writeln!(rep, "{s}"); }};
        }

        let b = load(&input);
        let rig = new_rig(b);
        let cm_clusters = rig.clusters.len();
        let avg_tris = rig.clusters.iter().map(|c| c.tri_count as f64).sum::<f64>() / cm_clusters as f64;
        say!("device {} | building: {} tris, {} verts, {} clusters ({:.1} tris/cluster avg)", rig.ctx.name(), rig.b.idx.len() / 3, rig.b.pos.len(), cm_clusters, avg_tris);
        say!("frames per timing run: {frames} (+20 warm-up) | {W}x{H} offscreen | GPU time from command buffer GPUStartTime/GPUEndTime");

        let r = rig.b.radius;
        let h = rig.b.center[1] * 2.0;

        // Snapshot 1: one building, close 3/4 view.
        let eye1 = Vec3::new(r * 1.8, h * 0.6 + r * 0.4, r * 2.0);
        let tgt1 = Vec3::new(0.0, h * 0.5, 0.0);
        let vp1 = view(eye1, tgt1);
        let s1 = scene(&rig, vec![[0.0; 4]]);
        say!("\n== correctness: one building, close view ==");
        let a1 = rig.draw(Path::Plain, &s1, vp1, eye1, true, 1.0);
        let r1 = rig.draw(Path::PlainRev, &s1, vp1, eye1, true, 1.0);
        let bn1 = rig.draw(Path::Clusters { cone: false }, &s1, vp1, eye1, true, 1.0);
        let bc1 = rig.draw(Path::Clusters { cone: true }, &s1, vp1, eye1, true, 1.0);
        report_snapshot(&rig, &mut rep, &out_dir, "single", &s1, vp1, eye1, &a1, &r1, &bn1, &bc1);

        // Distance tests: single building, camera at increasing distances.
        say!("\n== distance tests: single building, A (full detail) vs B (LOD) ==");
        for &thr in &[1.0f32, 2.0, 4.0, 16.0] {
            say!("  -- threshold {} px --", thr);
            for &dist in &[50.0f32, 200.0, 800.0] {
                let eye = Vec3::new(0.0, h * 0.5, dist);
                let tgt = Vec3::new(0.0, h * 0.5, 0.0);
                let vp = view(eye, tgt);
                let sc = scene(&rig, vec![[0.0; 4]]);
                let a = rig.draw(Path::Plain, &sc, vp, eye, true, thr);
                let b = rig.draw(Path::Clusters { cone: true }, &sc, vp, eye, true, thr);
                let (m, px, mx) = diff(&a.image, &b.image);
                let (refp, reft) = reference(&rig.clusters, &sc.offsets, &frustum_planes(vp.to_cols_array_2d()), eye, true, thr, H as f32, 50.0f32.to_radians());
                say!("  dist {:>4.0}m: A {} tris, B {} tris ({} pairs) | diff mean {:.4} px>8 {:.3}% max {} | ref pairs {} tris {}",
                    dist, a.tris, b.tris, b.pairs, m, px, mx, refp, reft);
            }
        }

        // Snapshot 2: grid of 400 instances, elevated view.
        let spacing = r * 2.0 + 28.0;
        let s2 = scene(&rig, grid(400, spacing));
        let half = ((s2.offsets.len() as f32).sqrt().ceil() - 1.0) * spacing * 0.5;
        let eye2 = Vec3::new(-half * 1.1, 80.0 + half * 0.25, half * 1.1);
        let vp2 = view(eye2, Vec3::ZERO);
        say!("\n== correctness: {} instances, elevated view ==", s2.offsets.len());
        let a2 = rig.draw(Path::Plain, &s2, vp2, eye2, true, 1.0);
        let r2 = rig.draw(Path::PlainRev, &s2, vp2, eye2, true, 1.0);
        let bn2 = rig.draw(Path::Clusters { cone: false }, &s2, vp2, eye2, true, 1.0);
        let bc2 = rig.draw(Path::Clusters { cone: true }, &s2, vp2, eye2, true, 1.0);
        report_snapshot(&rig, &mut rep, &out_dir, "grid400", &s2, vp2, eye2, &a2, &r2, &bn2, &bc2);

        // Timing sweep.
        say!("\n== timing: GPU ms per frame, median of {frames} (mean in brackets) ==");
        say!("{:>6} {:>22} {:>10} {:>9} {:>10} {:>12} {:>10}", "insts", "path", "gpu ms", "cpu ms", "draws", "pairs", "tris");
        for &n in &SIZES {
            let sc = scene(&rig, grid(n, spacing));
            let half = ((sc.offsets.len() as f32).sqrt().ceil() - 1.0) * spacing * 0.5;
            let (eye, tgt) = if n == 1 { (eye1, tgt1) } else { (Vec3::new(-half * 1.1, 80.0 + half * 0.25, half * 1.1), Vec3::ZERO) };
            let vp = view(eye, tgt);
            for path in [Path::Plain, Path::Clusters { cone: false }, Path::Clusters { cone: true }] {
                for _ in 0..20 {
                    rig.draw(path, &sc, vp, eye, false, 1.0);
                }
                let mut g = Vec::with_capacity(frames);
                let mut c = Vec::with_capacity(frames);
                let mut last = None;
                for _ in 0..frames {
                    let s = rig.draw(path, &sc, vp, eye, false, 1.0);
                    g.push(s.gpu_ms);
                    c.push(s.cpu_ms);
                    last = Some(s);
                }
                let mean = g.iter().sum::<f64>() / g.len() as f64;
                let last = last.unwrap();
                let med = median(&mut g);
                say!(
                    "{:>6} {:>22} {:>10.3} {:>9.3} {:>10} {:>12} {:>10}   [mean {:.3}]",
                    sc.offsets.len(),
                    path.label(),
                    med,
                    median(&mut c),
                    last.draws,
                    if matches!(path, Path::Plain) { "-".to_string() } else { last.pairs.to_string() },
                    last.tris,
                    mean
                );
            }
        }
        std::fs::write(out_dir.join("report.txt"), &rep).expect("write report");
        println!("\nwrote {}", out_dir.join("report.txt").display());
    }

    #[allow(clippy::too_many_arguments)]
    fn report_snapshot(rig: &Rig, rep: &mut String, out: &PathBuf, name: &str, sc: &Scene, vp: Mat4, eye: Vec3, a: &Shot, rv: &Shot, bn: &Shot, bc: &Shot) {
        let planes = frustum_planes(vp.to_cols_array_2d());
        let (ref_np, ref_nt) = reference(&rig.clusters, &sc.offsets, &planes, eye, false, 1.0, H as f32, 50.0f32.to_radians());
        let (ref_cp, ref_ct) = reference(&rig.clusters, &sc.offsets, &planes, eye, true, 1.0, H as f32, 50.0f32.to_radians());
        let lines = [
            format!("  pairs: GPU no-cone {} (CPU {}) | GPU cone {} (CPU {})", bn.pairs, ref_np, bc.pairs, ref_cp),
            format!("  tris:  GPU no-cone {} (CPU {}) | GPU cone {} (CPU {})", bn.tris, ref_nt, bc.tris, ref_ct),
        ];
        let mut block = String::new();
        for l in &lines {
            let _ = writeln!(block, "{l}");
        }
        let (c0, c1, c2) = diff(&a.image, &rv.image);
        let (m1, p1, x1) = diff(&a.image, &bn.image);
        let (m2, p2, x2) = diff(&a.image, &bc.image);
        let _ = write!(
            block,
            "  control A vs A(reversed order): mean abs {:.4}, pixels >8: {:.3}%, max {}\n  A vs B(no cone):   mean abs {:.4}, pixels >8: {:.3}%, max {}\n  A vs B(cone):      mean abs {:.4}, pixels >8: {:.3}%, max {}\n",
            c0, c1, c2, m1, p1, x1, m2, p2, x2
        );
        print!("{block}");
        rep.push_str(&block);
        write_ppm(&out.join(format!("{name}_A.ppm")), &a.image);
        write_ppm(&out.join(format!("{name}_A_reversed.ppm")), &rv.image);
        write_ppm(&out.join(format!("{name}_B_nocone.ppm")), &bn.image);
        write_ppm(&out.join(format!("{name}_B_cone.ppm")), &bc.image);
    }
}
