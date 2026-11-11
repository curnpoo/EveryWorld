//! Command recording on Metal.
//!
//! Synchronisation: Metal cannot see resources reached through bindless tables, so its own
//! hazard tracking misses them. Every Metal encoder (pass) updates its own fence when it
//! ends; when the caller records a transition outside a pass, the next encoder waits on the
//! fences of every encoder since the last wait (a full barrier, in Vulkan terms). Inside a
//! compute pass a transition is a `memoryBarrierWithScope`. Compute passes use the
//! concurrent dispatch type, so missing transitions show up on Metal too, as on Vulkan.

use super::*;

enum Enc {
    None,
    Render(P<dyn MTLRenderCommandEncoder>),
    Compute(P<dyn MTLComputeCommandEncoder>),
    Blit(P<dyn MTLBlitCommandEncoder>),
}

pub(crate) struct MetalEncoder<'d> {
    dev: &'d MetalDevice,
    frame: u64,
    slot: usize,
    cb: P<dyn MTLCommandBuffer>,
    /// Command buffers already committed this frame (submission timer path).
    committed: Vec<P<dyn MTLCommandBuffer>>,
    enc: Enc,
    tex_table: P<dyn MTLBuffer>,
    buf_table: P<dyn MTLBuffer>,
    smp_table: P<dyn MTLBuffer>,
    counters: Option<P<dyn MTLCounterSampleBuffer>>,
    /// A transition was recorded since the last encoder started: the next one waits.
    barrier: bool,
    encoders: usize,
    unwaited: Vec<P<dyn MTLFence>>,
    cur_fence: Option<P<dyn MTLFence>>,
    // Render pass state.
    pass_colors: Vec<Format>,
    pass_depth: Option<(Format, ResourceState)>,
    prim: Option<MTLPrimitiveType>,
    index: Option<(P<dyn MTLBuffer>, u64)>,
    group: Option<MTLSize>,
    // Timers and debug groups.
    timer_stack: Vec<(&'static str, usize)>,
    scopes: Vec<Scope>,
    passes: Vec<TimedPass>,
    samples: usize,
    /// For each open debug group: pushed on the current encoder (true) or the command buffer.
    debug: Vec<bool>,
    error: Option<String>,
    finished: bool,
}

type Fence = P<dyn MTLFence>;
/// Texture, format, width, height, depth (or 1), kind.
type TexInfo = (P<dyn MTLTexture>, Format, u32, u32, u32, TextureKind);

const ALL_STAGES: MTLRenderStages = MTLRenderStages::Vertex.union(MTLRenderStages::Fragment);

impl<'d> MetalEncoder<'d> {
    pub fn begin(dev: &'d MetalDevice) -> Result<Self> {
        let (frame, slot, uploads, unwaited, tex_table, buf_table, smp_table, counters) = {
            let mut st = dev.lock();
            let frame = st.frames_begun;
            let slot = (frame % dev.frames as u64) as usize;
            st.frames_begun += 1;
            st.frame_open = true;
            (
                frame,
                slot,
                std::mem::take(&mut st.uploads),
                std::mem::take(&mut st.tail_fences),
                st.tables.textures.clone(),
                st.tables.buffers[slot].clone(),
                st.tables.samplers.clone(),
                st.counters.get(slot).cloned(),
            )
        };
        let cb = dev.queue.commandBuffer().ok_or_else(|| Error::Backend("commandBuffer failed".into()))?;
        cb.setLabel(Some(&conv::ns(&format!("ed_gpu frame {frame}"))));
        let mut e = MetalEncoder {
            dev,
            frame,
            slot,
            cb,
            committed: Vec::new(),
            enc: Enc::None,
            tex_table,
            buf_table,
            smp_table,
            counters,
            // The first encoder waits for the previous frame's last encoders.
            barrier: true,
            encoders: 0,
            unwaited,
            cur_fence: None,
            pass_colors: Vec::new(),
            pass_depth: None,
            prim: None,
            index: None,
            group: None,
            timer_stack: Vec::new(),
            scopes: Vec::new(),
            passes: Vec::new(),
            samples: 0,
            debug: Vec::new(),
            error: None,
            finished: false,
        };
        if !uploads.is_empty() {
            e.apply_uploads(uploads);
        }
        Ok(e)
    }

    /// Always recorded (stale handles, misuse the API cannot express).
    fn fail(&mut self, msg: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(msg.into());
        }
    }

    /// State and usage checks: recorded only with validation on.
    fn check(&mut self, r: std::result::Result<(), String>, ctx: &str) {
        if let Err(e) = r
            && self.dev.validation
        {
            self.fail(format!("{ctx}: {e}"));
        }
    }

    fn timed(&self) -> bool {
        self.counters.is_some() && !self.timer_stack.is_empty() && self.passes.len() < MAX_TIMED_PASSES
    }

    fn reserve_samples(&mut self, n: usize) -> usize {
        let base = self.samples;
        self.samples += n;
        self.passes.push(TimedPass { base, samples: n });
        base
    }

    /// Fence for the encoder about to start; waits if a barrier is pending.
    fn next_fence(&mut self) -> Option<(P<dyn MTLFence>, Vec<Fence>)> {
        let fence = {
            let mut st = self.dev.lock();
            self.dev.fence(&mut st, self.slot, self.encoders)
        };
        let fence = match fence {
            Ok(f) => f,
            Err(e) => {
                self.fail(e.to_string());
                return None;
            }
        };
        self.encoders += 1;
        let waits = if self.barrier {
            self.barrier = false;
            std::mem::take(&mut self.unwaited)
        } else {
            Vec::new()
        };
        self.cur_fence = Some(fence.clone());
        Some((fence, waits))
    }

    fn end_encoder(&mut self) {
        let enc = std::mem::replace(&mut self.enc, Enc::None);
        if matches!(enc, Enc::None) {
            return;
        }
        // Debug groups pushed on this encoder must close with it.
        let mut unbalanced = false;
        while self.debug.last() == Some(&true) {
            self.debug.pop();
            unbalanced = true;
            match &enc {
                Enc::Render(e) => e.popDebugGroup(),
                Enc::Compute(e) => e.popDebugGroup(),
                Enc::Blit(e) => e.popDebugGroup(),
                Enc::None => {}
            }
        }
        if unbalanced {
            self.fail("a debug group opened inside a pass was not closed before the pass ended");
        }
        let fence = self.cur_fence.take();
        match &enc {
            Enc::Render(e) => {
                if let Some(f) = &fence {
                    e.updateFence_afterStages(f, ALL_STAGES);
                }
                e.endEncoding();
            }
            Enc::Compute(e) => {
                if let Some(f) = &fence {
                    e.updateFence(f);
                }
                e.endEncoding();
            }
            Enc::Blit(e) => {
                if let Some(f) = &fence {
                    e.updateFence(f);
                }
                e.endEncoding();
            }
            Enc::None => {}
        }
        if let Some(f) = fence {
            self.unwaited.push(f);
        }
        self.prim = None;
        self.index = None;
        self.group = None;
        self.pass_colors.clear();
        self.pass_depth = None;
    }

    /// Ends a blit encoder; a render or compute pass still open is an error.
    fn leave_pass(&mut self, what: &str) {
        if matches!(self.enc, Enc::Render(_) | Enc::Compute(_)) {
            self.fail(format!("{what}: a pass is still open (passes do not nest)"));
        }
        self.end_encoder();
    }

    fn blit(&mut self, what: &str) -> Option<P<dyn MTLBlitCommandEncoder>> {
        match &self.enc {
            Enc::Blit(b) => return Some(b.clone()),
            Enc::Render(_) | Enc::Compute(_) => {
                self.fail(format!("{what}: copies are not allowed inside a pass"));
                return None;
            }
            Enc::None => {}
        }
        let (fence, waits) = self.next_fence()?;
        let d = MTLBlitPassDescriptor::new();
        if self.timed() {
            let base = self.reserve_samples(2);
            // SAFETY: index 0 of the fixed attachment array; sample indices are within the
            // buffer's allocated count (bounded by MAX_TIMED_PASSES).
            unsafe {
                let a = d.sampleBufferAttachments().objectAtIndexedSubscript(0);
                a.setSampleBuffer(self.counters.as_deref());
                a.setStartOfEncoderSampleIndex(base);
                a.setEndOfEncoderSampleIndex(base + 1);
            }
        }
        let Some(b) = self.cb.blitCommandEncoderWithDescriptor(&d) else {
            self.fail("blitCommandEncoder failed");
            return None;
        };
        b.setLabel(Some(&conv::ns("copies")));
        for w in &waits {
            b.waitForFence(w);
        }
        let _ = fence;
        self.enc = Enc::Blit(b.clone());
        Some(b)
    }

    fn apply_uploads(&mut self, uploads: Vec<Upload>) {
        let Some(blit) = self.blit("uploads") else { return };
        blit.pushDebugGroup(&conv::ns("uploads"));
        for u in uploads {
            let mut st = self.dev.lock();
            match u {
                Upload::Buffer { staging, dst, offset } => {
                    let Some(b) = st.buffers.get_mut(dst.index(), dst.generation()) else { continue };
                    let r = b.states.expect(None, &[ResourceState::CopyDst, ResourceState::Undefined]);
                    b.states.set(None, ResourceState::CopyDst);
                    let raw = b.raws[0].clone();
                    drop(st);
                    self.check(r, "write_buffer (applied at frame start)");
                    // SAFETY: range validated in write_buffer; both buffers are alive.
                    unsafe { blit.copyFromBuffer_sourceOffset_toBuffer_destinationOffset_size(&staging, 0, &raw, offset as usize, staging.length()) };
                }
                Upload::Texture { staging, dst, sub } => {
                    let Some(t) = st.textures.get_mut(dst.index(), dst.generation()) else { continue };
                    let r = t.states.expect(Some(sub), &[ResourceState::CopyDst, ResourceState::Undefined]);
                    t.states.set(Some(sub), ResourceState::CopyDst);
                    let (w, h) = (mip_size(t.width, sub.mip), mip_size(t.height, sub.mip));
                    let d = if t.kind == TextureKind::D3 { mip_size(t.depth(), sub.mip) } else { 1 };
                    let row = t.format.packed_row_bytes(w) as usize;
                    let image = row * t.format.block_rows(h) as usize;
                    let raw = t.raw.clone();
                    drop(st);
                    self.check(r, "write_texture (applied at frame start)");
                    // SAFETY: the staging size was checked against this subresource in
                    // write_texture; slice and level are in range.
                    unsafe {
                        blit.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                            &staging,
                            0,
                            row,
                            image,
                            MTLSize { width: w as usize, height: h as usize, depth: d as usize },
                            &raw,
                            sub.layer as usize,
                            sub.mip as usize,
                            MTLOrigin { x: 0, y: 0, z: 0 },
                        )
                    };
                }
            }
        }
        blit.popDebugGroup();
    }

    fn texture_info(&mut self, id: TextureId, what: &str) -> Option<TexInfo> {
        let st = self.dev.lock();
        match st.textures.get(id.index(), id.generation()) {
            Some(t) => Some((t.raw.clone(), t.format, t.width, t.height, t.depth(), t.kind)),
            None => {
                drop(st);
                self.fail(format!("{what}: stale texture"));
                None
            }
        }
    }

    /// Checks a texture subresource state; returns false (and records) on a stale handle.
    fn expect_tex(&mut self, id: TextureId, sub: Option<Subresource>, allowed: &[ResourceState], what: &str) -> bool {
        let r = {
            let st = self.dev.lock();
            match st.textures.get(id.index(), id.generation()) {
                Some(t) => t.states.expect(sub, allowed),
                None => {
                    drop(st);
                    self.fail(format!("{what}: stale texture"));
                    return false;
                }
            }
        };
        self.check(r, what);
        true
    }

    /// Checks a buffer state and returns this frame's Metal buffer and its size.
    fn buffer(&mut self, id: BufferId, allowed: &[ResourceState], what: &str) -> Option<(P<dyn MTLBuffer>, u64)> {
        let (r, raw, size) = {
            let st = self.dev.lock();
            match st.buffers.get(id.index(), id.generation()) {
                Some(b) => (b.states.expect(None, allowed), b.raws[self.slot % b.raws.len()].clone(), b.size),
                None => {
                    drop(st);
                    self.fail(format!("{what}: stale buffer"));
                    return None;
                }
            }
        };
        self.check(r, what);
        Some((raw, size))
    }

    fn render(&mut self, what: &str) -> Option<P<dyn MTLRenderCommandEncoder>> {
        match &self.enc {
            Enc::Render(r) => Some(r.clone()),
            _ => {
                self.fail(format!("{what}: needs an open render pass"));
                None
            }
        }
    }

    fn compute(&mut self, what: &str) -> Option<P<dyn MTLComputeCommandEncoder>> {
        match &self.enc {
            Enc::Compute(c) => Some(c.clone()),
            _ => {
                self.fail(format!("{what}: needs an open compute pass"));
                None
            }
        }
    }

    /// Commits the current command buffer and starts a new one (submission timer path).
    fn split(&mut self) {
        self.end_encoder();
        self.commit_residency();
        let Some(next) = self.dev.queue.commandBuffer() else {
            self.fail("commandBuffer failed");
            return;
        };
        next.setLabel(Some(&conv::ns(&format!("ed_gpu frame {} part {}", self.frame, self.committed.len() + 1))));
        let prev = std::mem::replace(&mut self.cb, next);
        prev.commit();
        self.committed.push(prev);
    }

    fn commit_residency(&self) {
        let mut st = self.dev.lock();
        if st.residency_dirty {
            st.residency.commit();
            st.residency_dirty = false;
        }
    }

    fn do_finish(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        if matches!(self.enc, Enc::Render(_) | Enc::Compute(_)) {
            self.fail("end_frame: a pass is still open");
        }
        self.end_encoder();
        if !self.timer_stack.is_empty() {
            self.fail("end_frame: timer scopes still open");
            while self.timer_stack.pop().is_some() {}
        }
        while let Some(on_encoder) = self.debug.pop() {
            if !on_encoder {
                self.cb.popDebugGroup();
            }
            self.fail("end_frame: debug groups still open");
        }
        self.commit_residency();
        let mut st = self.dev.lock();
        for (drawable, id) in std::mem::take(&mut st.acquired) {
            if let Some(t) = st.textures.remove(id.index(), id.generation()) {
                let r = t.states.expect(None, &[ResourceState::Present]);
                if let Err(e) = r
                    && self.dev.validation
                    && self.error.is_none()
                {
                    self.error = Some(format!("end_frame: surface texture must be transitioned to Present: {e}"));
                }
            }
            self.cb.presentDrawable(ProtocolObject::from_ref(&*drawable));
        }
        self.cb.commit();
        let mut cbs = std::mem::take(&mut self.committed);
        cbs.push(self.cb.clone());
        st.in_flight.push_back(FrameRec { index: self.frame, cbs, slot: self.slot, passes: std::mem::take(&mut self.passes), scopes: std::mem::take(&mut self.scopes) });
        st.tail_fences = std::mem::take(&mut self.unwaited);
        st.frame_open = false;
        self.dev.retire(&mut st);
        drop(st);
        match self.error.take() {
            Some(e) => Err(Error::Invalid(e)),
            None => Ok(()),
        }
    }
}

impl Drop for MetalEncoder<'_> {
    fn drop(&mut self) {
        // An encoder dropped without end_frame still submits, so frame accounting stays whole.
        let _ = self.do_finish();
    }
}

impl CommandEncoder for MetalEncoder<'_> {
    fn frame_index(&self) -> u64 {
        self.frame
    }

    fn transition(&mut self, transitions: &[Transition]) {
        if matches!(self.enc, Enc::Render(_)) {
            self.fail("transition: not allowed inside a render pass");
            return;
        }
        let mut st = self.dev.lock();
        let mut errs = Vec::new();
        for t in transitions {
            match *t {
                Transition::Buffer { buffer, from, to } => match st.buffers.get_mut(buffer.index(), buffer.generation()) {
                    Some(b) => {
                        if !usage_allows(&b.usage, to, false) {
                            errs.push((true, format!("transition: buffer usage does not allow {to:?}")));
                        }
                        if let Err(e) = b.states.transition(None, from, to) {
                            errs.push((true, format!("transition (buffer): {e}")));
                        }
                    }
                    None => errs.push((false, "transition: stale buffer".to_string())),
                },
                Transition::Texture { texture, sub, from, to } => match st.textures.get_mut(texture.index(), texture.generation()) {
                    Some(tex) => {
                        if !usage_allows(&tex.usage, to, tex.surface) {
                            errs.push((true, format!("transition: texture usage does not allow {to:?}")));
                        }
                        if let Err(e) = tex.states.transition(sub, from, to) {
                            errs.push((true, format!("transition (texture): {e}")));
                        }
                    }
                    None => errs.push((false, "transition: stale texture".to_string())),
                },
            }
        }
        drop(st);
        for (is_state, e) in errs {
            if is_state {
                self.check(Err(e), "transition");
            } else {
                self.fail(e);
            }
        }
        match &self.enc {
            Enc::Compute(c) => c.memoryBarrierWithScope(MTLBarrierScope::Buffers | MTLBarrierScope::Textures),
            Enc::Blit(_) => {
                self.end_encoder();
                self.barrier = true;
            }
            _ => self.barrier = true,
        }
    }

    fn begin_render_pass(&mut self, desc: &RenderPassDesc<'_>) {
        self.leave_pass("begin_render_pass");
        if desc.colors.len() > 8 {
            self.fail("begin_render_pass: more than 8 colour attachments");
            return;
        }
        let rp = MTLRenderPassDescriptor::new();
        let mut colors = Vec::new();
        for (i, a) in desc.colors.iter().enumerate() {
            if !self.expect_tex(a.texture, Some(a.sub), &[ResourceState::RenderTarget], "begin_render_pass (colour)") {
                return;
            }
            let Some((raw, format, ..)) = self.texture_info(a.texture, "begin_render_pass") else { return };
            colors.push(format);
            // SAFETY: i < 8, within the attachment array.
            let att = unsafe { rp.colorAttachments().objectAtIndexedSubscript(i) };
            att.setTexture(Some(&raw));
            set_sub(&att, a.sub, &raw);
            att.setLoadAction(conv::load(a.load));
            att.setStoreAction(conv::store(a.store));
            if let LoadOp::Clear(c) = a.load {
                att.setClearColor(MTLClearColor { red: c[0] as f64, green: c[1] as f64, blue: c[2] as f64, alpha: c[3] as f64 });
            }
        }
        let mut depth = None;
        if let Some(a) = &desc.depth {
            let state = {
                let st = self.dev.lock();
                st.textures.get(a.texture.index(), a.texture.generation()).map(|t| t.states.expect(Some(a.sub), &[ResourceState::DepthWrite]).is_ok())
            };
            if !self.expect_tex(a.texture, Some(a.sub), &[ResourceState::DepthWrite, ResourceState::DepthRead], "begin_render_pass (depth)") {
                return;
            }
            let Some((raw, format, ..)) = self.texture_info(a.texture, "begin_render_pass") else { return };
            depth = Some((format, if state == Some(true) { ResourceState::DepthWrite } else { ResourceState::DepthRead }));
            let att = rp.depthAttachment();
            att.setTexture(Some(&raw));
            set_sub(&att, a.sub, &raw);
            att.setLoadAction(conv::load(a.load));
            att.setStoreAction(conv::store(a.store));
            if let LoadOp::Clear(c) = a.load {
                att.setClearDepth(c[0] as f64);
            }
        }
        if self.timed() {
            let base = self.reserve_samples(4);
            // SAFETY: index 0 of the fixed attachment array; indices within the sample count.
            unsafe {
                let s = rp.sampleBufferAttachments().objectAtIndexedSubscript(0);
                s.setSampleBuffer(self.counters.as_deref());
                s.setStartOfVertexSampleIndex(base);
                s.setStartOfFragmentSampleIndex(base + 1);
                s.setEndOfVertexSampleIndex(base + 2);
                s.setEndOfFragmentSampleIndex(base + 3);
            }
        }
        let Some((_, waits)) = self.next_fence() else { return };
        let Some(r) = self.cb.renderCommandEncoderWithDescriptor(&rp) else {
            self.fail("begin_render_pass: renderCommandEncoder failed");
            return;
        };
        r.setLabel(Some(&conv::ns(desc.label)));
        for w in &waits {
            r.waitForFence_beforeStages(w, ALL_STAGES);
        }
        // SAFETY: the tables are alive for the frame (held by this encoder); the constants
        // pointer is to 64 live bytes.
        unsafe {
            for (buf, i) in [(&self.tex_table, 1), (&self.buf_table, 2), (&self.smp_table, 3)] {
                r.setVertexBuffer_offset_atIndex(Some(buf), 0, i);
                r.setFragmentBuffer_offset_atIndex(Some(buf), 0, i);
            }
            let z = NonNull::from(&self.dev.zero_constants).cast();
            r.setVertexBytes_length_atIndex(z, DRAW_CONSTANTS_BYTES, 0);
            r.setFragmentBytes_length_atIndex(z, DRAW_CONSTANTS_BYTES, 0);
        }
        r.setFrontFacingWinding(MTLWinding::CounterClockwise);
        self.pass_colors = colors;
        self.pass_depth = depth;
        self.enc = Enc::Render(r);
    }

    fn end_render_pass(&mut self) {
        if !matches!(self.enc, Enc::Render(_)) {
            self.fail("end_render_pass: no render pass is open");
            return;
        }
        self.end_encoder();
    }

    fn set_viewport(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if let Some(r) = self.render("set_viewport") {
            r.setViewport(MTLViewport { originX: x as f64, originY: y as f64, width: w as f64, height: h as f64, znear: 0.0, zfar: 1.0 });
        }
    }

    fn set_scissor(&mut self, x: u32, y: u32, w: u32, h: u32) {
        if let Some(r) = self.render("set_scissor") {
            r.setScissorRect(MTLScissorRect { x: x as usize, y: y as usize, width: w as usize, height: h as usize });
        }
    }

    fn set_render_pipeline(&mut self, pipeline: RenderPipelineId) {
        let Some(r) = self.render("set_render_pipeline") else { return };
        let p = {
            let st = self.dev.lock();
            st.rpipes.get(pipeline.index(), pipeline.generation()).map(|p| (p.pso.clone(), p.depth_state.clone(), p.prim, p.cull, p.bias, p.colors.clone(), p.depth))
        };
        let Some((pso, ds, prim, cull, bias, colors, depth)) = p else {
            self.fail("set_render_pipeline: stale pipeline");
            return;
        };
        if colors != self.pass_colors || depth.map(|d| d.0) != self.pass_depth.map(|d| d.0) {
            self.fail(format!("set_render_pipeline: pipeline targets {colors:?}/{depth:?} do not match the pass {:?}/{:?}", self.pass_colors, self.pass_depth));
            return;
        }
        if let (Some((_, true)), Some((_, ResourceState::DepthRead))) = (depth, self.pass_depth) {
            self.check(Err("pipeline writes depth but the depth attachment is in DepthRead".into()), "set_render_pipeline");
        }
        r.setRenderPipelineState(&pso);
        r.setCullMode(cull);
        if let Some(ds) = &ds {
            r.setDepthStencilState(Some(ds));
            r.setDepthBias_slopeScale_clamp(bias.0, bias.1, 0.0);
        }
        self.prim = Some(prim);
    }

    fn set_index_buffer(&mut self, buffer: BufferId, offset: u64) {
        if self.render("set_index_buffer").is_none() {
            return;
        }
        if !offset.is_multiple_of(4) {
            self.fail("set_index_buffer: offset must be a multiple of 4");
            return;
        }
        if let Some((raw, _)) = self.buffer(buffer, &[ResourceState::ShaderRead], "set_index_buffer") {
            self.index = Some((raw, offset));
        }
    }

    fn set_draw_constants(&mut self, data: &[u8; DRAW_CONSTANTS_BYTES]) {
        if let Some(r) = self.render("set_draw_constants") {
            // SAFETY: Metal copies the 64 bytes now.
            unsafe {
                let p = NonNull::from(data).cast();
                r.setVertexBytes_length_atIndex(p, DRAW_CONSTANTS_BYTES, 0);
                r.setFragmentBytes_length_atIndex(p, DRAW_CONSTANTS_BYTES, 0);
            }
        }
    }

    fn draw(&mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>) {
        let Some(r) = self.render("draw") else { return };
        let Some(prim) = self.prim else { return self.fail("draw: no pipeline set") };
        if vertices.is_empty() || instances.is_empty() {
            return;
        }
        // SAFETY: a pipeline is set; counts are plain integers.
        unsafe {
            r.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(prim, vertices.start as usize, vertices.len(), instances.len(), instances.start as usize)
        };
    }

    fn draw_indexed(&mut self, indices: std::ops::Range<u32>, base_vertex: i32, instances: std::ops::Range<u32>) {
        let Some(r) = self.render("draw_indexed") else { return };
        let Some(prim) = self.prim else { return self.fail("draw_indexed: no pipeline set") };
        let Some((ib, off)) = self.index.clone() else { return self.fail("draw_indexed: no index buffer set") };
        if indices.is_empty() || instances.is_empty() {
            return;
        }
        // SAFETY: the index buffer is alive (held here); Metal validates ranges in debug.
        unsafe {
            r.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount_baseVertex_baseInstance(
                prim,
                indices.len(),
                MTLIndexType::UInt32,
                &ib,
                off as usize + indices.start as usize * 4,
                instances.len(),
                base_vertex as isize,
                instances.start as usize,
            )
        };
    }

    fn draw_indexed_indirect_multi(&mut self, args: BufferId, args_offset: u64, max_draws: u32, count: Option<(BufferId, u64)>) {
        let Some(r) = self.render("draw_indexed_indirect_multi") else { return };
        let Some(prim) = self.prim else { return self.fail("draw_indexed_indirect_multi: no pipeline set") };
        let Some((ib, off)) = self.index.clone() else { return self.fail("draw_indexed_indirect_multi: no index buffer set") };
        let Some((ab, size)) = self.buffer(args, &[ResourceState::IndirectArgs], "draw_indexed_indirect_multi (args)") else { return };
        if let Some((cbuf, _)) = count {
            let _ = self.buffer(cbuf, &[ResourceState::IndirectArgs], "draw_indexed_indirect_multi (count)");
        }
        let stride = std::mem::size_of::<DrawIndexedArgs>() as u64;
        if !args_offset.is_multiple_of(4) || args_offset + stride * max_draws as u64 > size {
            return self.fail("draw_indexed_indirect_multi: args range outside the buffer or unaligned");
        }
        // Metal has no multi-draw-indirect: one indirect draw per entry. The count is not
        // honoured (Caps::indirect_count == false), so entries past it must be zeroed.
        for i in 0..max_draws as u64 {
            // SAFETY: the range was checked above; buffers are alive (held here).
            unsafe {
                r.drawIndexedPrimitives_indexType_indexBuffer_indexBufferOffset_indirectBuffer_indirectBufferOffset(
                    prim,
                    MTLIndexType::UInt32,
                    &ib,
                    off as usize,
                    &ab,
                    (args_offset + i * stride) as usize,
                )
            };
        }
    }

    fn begin_compute_pass(&mut self, label: &str) {
        self.leave_pass("begin_compute_pass");
        let d = MTLComputePassDescriptor::new();
        d.setDispatchType(MTLDispatchType::Concurrent);
        if self.timed() {
            let base = self.reserve_samples(2);
            // SAFETY: index 0 of the fixed attachment array; indices within the sample count.
            unsafe {
                let a = d.sampleBufferAttachments().objectAtIndexedSubscript(0);
                a.setSampleBuffer(self.counters.as_deref());
                a.setStartOfEncoderSampleIndex(base);
                a.setEndOfEncoderSampleIndex(base + 1);
            }
        }
        let Some((_, waits)) = self.next_fence() else { return };
        let Some(c) = self.cb.computeCommandEncoderWithDescriptor(&d) else {
            return self.fail("begin_compute_pass: computeCommandEncoder failed");
        };
        c.setLabel(Some(&conv::ns(label)));
        for w in &waits {
            c.waitForFence(w);
        }
        // SAFETY: tables alive for the frame; constants pointer to 64 live bytes.
        unsafe {
            c.setBuffer_offset_atIndex(Some(&self.tex_table), 0, 1);
            c.setBuffer_offset_atIndex(Some(&self.buf_table), 0, 2);
            c.setBuffer_offset_atIndex(Some(&self.smp_table), 0, 3);
            c.setBytes_length_atIndex(NonNull::from(&self.dev.zero_constants).cast(), DRAW_CONSTANTS_BYTES, 0);
        }
        self.enc = Enc::Compute(c);
    }

    fn end_compute_pass(&mut self) {
        if !matches!(self.enc, Enc::Compute(_)) {
            return self.fail("end_compute_pass: no compute pass is open");
        }
        self.end_encoder();
    }

    fn set_compute_pipeline(&mut self, pipeline: ComputePipelineId) {
        let Some(c) = self.compute("set_compute_pipeline") else { return };
        let p = {
            let st = self.dev.lock();
            st.cpipes.get(pipeline.index(), pipeline.generation()).map(|p| (p.pso.clone(), p.group))
        };
        let Some((pso, group)) = p else { return self.fail("set_compute_pipeline: stale pipeline") };
        c.setComputePipelineState(&pso);
        self.group = Some(group);
    }

    fn set_compute_constants(&mut self, data: &[u8; DRAW_CONSTANTS_BYTES]) {
        if let Some(c) = self.compute("set_compute_constants") {
            // SAFETY: Metal copies the 64 bytes now.
            unsafe { c.setBytes_length_atIndex(NonNull::from(data).cast(), DRAW_CONSTANTS_BYTES, 0) };
        }
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        let Some(c) = self.compute("dispatch") else { return };
        let Some(group) = self.group else { return self.fail("dispatch: no pipeline set") };
        if x == 0 || y == 0 || z == 0 {
            return;
        }
        c.dispatchThreadgroups_threadsPerThreadgroup(MTLSize { width: x as usize, height: y as usize, depth: z as usize }, group);
    }

    fn dispatch_indirect(&mut self, args: BufferId, offset: u64) {
        let Some(c) = self.compute("dispatch_indirect") else { return };
        let Some(group) = self.group else { return self.fail("dispatch_indirect: no pipeline set") };
        let Some((ab, size)) = self.buffer(args, &[ResourceState::IndirectArgs], "dispatch_indirect") else { return };
        if !offset.is_multiple_of(4) || offset + 12 > size {
            return self.fail("dispatch_indirect: args outside the buffer or unaligned");
        }
        // SAFETY: range checked; buffer alive.
        unsafe { c.dispatchThreadgroupsWithIndirectBuffer_indirectBufferOffset_threadsPerThreadgroup(&ab, offset as usize, group) };
    }

    fn copy_buffer(&mut self, src: BufferId, src_offset: u64, dst: BufferId, dst_offset: u64, size: u64) {
        let Some((s, ss)) = self.buffer(src, &[ResourceState::CopySrc], "copy_buffer (src)") else { return };
        let Some((d, ds)) = self.buffer(dst, &[ResourceState::CopyDst], "copy_buffer (dst)") else { return };
        if src_offset + size > ss || dst_offset + size > ds {
            return self.fail("copy_buffer: range outside a buffer");
        }
        let Some(b) = self.blit("copy_buffer") else { return };
        // SAFETY: ranges checked; buffers alive.
        unsafe { b.copyFromBuffer_sourceOffset_toBuffer_destinationOffset_size(&s, src_offset as usize, &d, dst_offset as usize, size as usize) };
    }

    fn copy_texture(&mut self, src: TextureId, src_sub: Subresource, dst: TextureId, dst_sub: Subresource) {
        if !self.expect_tex(src, Some(src_sub), &[ResourceState::CopySrc], "copy_texture (src)") || !self.expect_tex(dst, Some(dst_sub), &[ResourceState::CopyDst], "copy_texture (dst)") {
            return;
        }
        let Some((s, sf, sw, sh, sd, sk)) = self.texture_info(src, "copy_texture") else { return };
        let Some((d, df, dw, dh, ..)) = self.texture_info(dst, "copy_texture") else { return };
        let size = (mip_size(sw, src_sub.mip), mip_size(sh, src_sub.mip));
        if sf != df || size != (mip_size(dw, dst_sub.mip), mip_size(dh, dst_sub.mip)) {
            return self.fail("copy_texture: formats or subresource sizes differ");
        }
        let depth = if sk == TextureKind::D3 { mip_size(sd, src_sub.mip) } else { 1 };
        let Some(b) = self.blit("copy_texture") else { return };
        // SAFETY: subresources validated against the textures; sizes match.
        unsafe {
            b.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                &s,
                if sk == TextureKind::D3 { 0 } else { src_sub.layer as usize },
                src_sub.mip as usize,
                MTLOrigin { x: 0, y: 0, z: 0 },
                MTLSize { width: size.0 as usize, height: size.1 as usize, depth: depth as usize },
                &d,
                dst_sub.layer as usize,
                dst_sub.mip as usize,
                MTLOrigin { x: 0, y: 0, z: 0 },
            )
        };
    }

    fn copy_texture_to_buffer(&mut self, src: TextureId, sub: Subresource, dst: BufferId, dst_offset: u64, bytes_per_row: u32) {
        if !self.expect_tex(src, Some(sub), &[ResourceState::CopySrc], "copy_texture_to_buffer (src)") {
            return;
        }
        let Some((s, f, w, h, d, k)) = self.texture_info(src, "copy_texture_to_buffer") else { return };
        let Some((b, bs)) = self.buffer(dst, &[ResourceState::CopyDst], "copy_texture_to_buffer (dst)") else { return };
        let (w, h) = (mip_size(w, sub.mip), mip_size(h, sub.mip));
        let depth = if k == TextureKind::D3 { mip_size(d, sub.mip) } else { 1 };
        if !bytes_per_row.is_multiple_of(COPY_ROW_PITCH_ALIGN) || bytes_per_row < f.packed_row_bytes(w) {
            return self.fail(format!("copy_texture_to_buffer: bytes_per_row {bytes_per_row} must be a multiple of {COPY_ROW_PITCH_ALIGN} and at least {}", f.packed_row_bytes(w)));
        }
        let image = bytes_per_row as u64 * f.block_rows(h) as u64;
        if dst_offset + image * depth as u64 > bs {
            return self.fail("copy_texture_to_buffer: buffer too small");
        }
        let Some(bl) = self.blit("copy_texture_to_buffer") else { return };
        // SAFETY: sizes and the destination range validated above.
        unsafe {
            bl.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(
                &s,
                if k == TextureKind::D3 { 0 } else { sub.layer as usize },
                sub.mip as usize,
                MTLOrigin { x: 0, y: 0, z: 0 },
                MTLSize { width: w as usize, height: h as usize, depth: depth as usize },
                &b,
                dst_offset as usize,
                bytes_per_row as usize,
                image as usize,
            )
        };
    }

    fn generate_mips(&mut self, texture: TextureId) {
        if !self.expect_tex(texture, None, &[ResourceState::CopyDst], "generate_mips") {
            return;
        }
        let Some((t, ..)) = self.texture_info(texture, "generate_mips") else { return };
        if let Some(b) = self.blit("generate_mips") {
            b.generateMipmapsForTexture(&t);
        }
    }

    fn begin_timer(&mut self, label: &'static str) {
        if matches!(self.enc, Enc::Render(_) | Enc::Compute(_)) {
            return self.fail("begin_timer: timer scopes go around passes, not inside them");
        }
        if self.counters.is_some() {
            self.end_encoder(); // a following copy gets its own timed blit encoder
            self.timer_stack.push((label, self.passes.len()));
        } else {
            self.split();
            self.timer_stack.push((label, self.committed.len()));
        }
    }

    fn end_timer(&mut self) {
        if matches!(self.enc, Enc::Render(_) | Enc::Compute(_)) {
            return self.fail("end_timer: timer scopes go around passes, not inside them");
        }
        let Some((label, first)) = self.timer_stack.pop() else { return self.fail("end_timer without begin_timer") };
        let depth = self.timer_stack.len() as u32;
        if self.counters.is_some() {
            self.end_encoder();
            self.scopes.push(Scope { label, depth, first, end: self.passes.len() });
        } else {
            self.split();
            self.scopes.push(Scope { label, depth, first, end: self.committed.len() });
        }
    }

    fn debug_group(&mut self, label: &str) {
        let s = conv::ns(label);
        let on_encoder = match &self.enc {
            Enc::Render(e) => {
                e.pushDebugGroup(&s);
                true
            }
            Enc::Compute(e) => {
                e.pushDebugGroup(&s);
                true
            }
            Enc::Blit(_) | Enc::None => {
                // Copies started later must not inherit an encoder-level group.
                self.end_encoder();
                self.cb.pushDebugGroup(&s);
                false
            }
        };
        self.debug.push(on_encoder);
    }

    fn end_debug_group(&mut self) {
        match self.debug.pop() {
            None => self.fail("end_debug_group without debug_group"),
            Some(true) => match &self.enc {
                Enc::Render(e) => e.popDebugGroup(),
                Enc::Compute(e) => e.popDebugGroup(),
                _ => self.fail("end_debug_group: the group was opened inside a pass that has ended"),
            },
            Some(false) => {
                if matches!(self.enc, Enc::Render(_) | Enc::Compute(_)) {
                    self.fail("end_debug_group: a group opened outside a pass must close outside it");
                    return;
                }
                self.end_encoder();
                self.cb.popDebugGroup();
            }
        }
    }

    fn finish(mut self: Box<Self>) -> Result<()> {
        self.do_finish()
    }
}

/// Sets level and slice (or depth plane for 3D textures) on an attachment.
fn set_sub(att: &MTLRenderPassAttachmentDescriptor, sub: Subresource, raw: &ProtocolObject<dyn MTLTexture>) {
    att.setLevel(sub.mip as usize);
    if raw.textureType() == MTLTextureType::Type3D {
        att.setDepthPlane(sub.layer as usize);
    } else {
        att.setSlice(sub.layer as usize);
    }
}
