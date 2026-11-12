//! Vulkan command encoder: one primary command buffer per frame, dynamic rendering, explicit
//! barriers built from the tracked states.

use std::ffi::CString;

use ash::vk;

use super::conv::{self, Access, Res};
use super::{FrameRec, MAX_QUERIES, Scope, Upload, VulkanDevice, range};
use crate::track::usage_allows;
use crate::*;

enum Pass {
    None,
    Render { colors: Vec<Format>, depth: Option<(Format, ResourceState)> },
    Compute,
}

pub(crate) struct VulkanEncoder<'d> {
    dev: &'d VulkanDevice,
    frame: u64,
    slot: usize,
    cb: vk::CommandBuffer,
    queries: vk::QueryPool,
    set: vk::DescriptorSet,
    pass: Pass,
    pipeline: bool,
    index: bool,
    compute_pipeline: bool,
    next_query: u32,
    timer_stack: Vec<(&'static str, Option<u32>)>,
    scopes: Vec<Scope>,
    /// For each open debug group: opened inside a pass.
    debug: Vec<bool>,
    error: Option<String>,
    finished: bool,
}

/// Texture data an encoder command needs, copied out under the lock.
#[derive(Clone)]
struct TexInfo {
    image: vk::Image,
    aspect: vk::ImageAspectFlags,
    format: Format,
    width: u32,
    height: u32,
    depth: u32,
    kind: TextureKind,
    mips: u32,
    layers: u32,
    att_views: Vec<vk::ImageView>,
}

const ZERO: [u8; DRAW_CONSTANTS_BYTES] = [0; DRAW_CONSTANTS_BYTES];

fn all_stages() -> vk::ShaderStageFlags {
    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT | vk::ShaderStageFlags::COMPUTE
}

fn image_barrier(image: vk::Image, aspect: vk::ImageAspectFlags, sub: vk::ImageSubresourceRange, src: Access, old: vk::ImageLayout, dst: Access) -> vk::ImageMemoryBarrier2<'static> {
    let _ = aspect;
    vk::ImageMemoryBarrier2::default()
        .src_stage_mask(src.stages)
        .src_access_mask(src.src_access())
        .dst_stage_mask(dst.stages)
        .dst_access_mask(dst.access)
        .old_layout(old)
        .new_layout(dst.layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(sub)
}

fn buffer_barrier(buffer: vk::Buffer, src: Access, dst: Access) -> vk::BufferMemoryBarrier2<'static> {
    vk::BufferMemoryBarrier2::default()
        .src_stage_mask(src.stages)
        .src_access_mask(src.src_access())
        .dst_stage_mask(dst.stages)
        .dst_access_mask(dst.access)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .buffer(buffer)
        .offset(0)
        .size(vk::WHOLE_SIZE)
}

impl<'d> VulkanEncoder<'d> {
    pub fn begin(dev: &'d VulkanDevice) -> Result<Self> {
        let (frame, slot, uploads, cb, pool, queries, set) = {
            let mut st = dev.lock();
            let frame = st.frames_begun;
            let slot = (frame % dev.frames as u64) as usize;
            let s = &st.slots[slot];
            let (cb, pool, queries, set) = (s.cb, s.pool, s.queries, s.set);
            st.frames_begun += 1;
            st.frame_open = true;
            (frame, slot, std::mem::take(&mut st.uploads), cb, pool, queries, set)
        };
        let mut e = VulkanEncoder {
            dev,
            frame,
            slot,
            cb,
            queries,
            set,
            pass: Pass::None,
            pipeline: false,
            index: false,
            compute_pipeline: false,
            next_query: 0,
            timer_stack: Vec::new(),
            scopes: Vec::new(),
            debug: Vec::new(),
            error: None,
            finished: false,
        };
        // SAFETY: this slot's previous frame has finished (begin_frame paced on the timeline),
        // so its pool and command buffer are idle; only this encoder records into them.
        let r = unsafe {
            dev.raw
                .reset_command_pool(pool, vk::CommandPoolResetFlags::empty())
                .and_then(|_| dev.raw.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)))
        };
        if let Err(err) = r {
            // Keep frame accounting whole: the frame is never submitted, so close it here.
            let mut st = dev.lock();
            st.frame_open = false;
            st.frames_begun -= 1;
            e.finished = true;
            return Err(super::vkr(err));
        }
        // SAFETY: recording; the pool has MAX_QUERIES queries.
        unsafe { dev.raw.cmd_reset_query_pool(cb, queries, 0, MAX_QUERIES) };
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

    fn in_pass(&self) -> bool {
        !matches!(self.pass, Pass::None)
    }

    fn label(&self, text: &str) {
        if let Some(du) = &self.dev.exts.debug_utils
            && let Ok(c) = CString::new(text)
        {
            // SAFETY: recording command buffer; the label is copied.
            unsafe { du.cmd_begin_debug_utils_label(self.cb, &vk::DebugUtilsLabelEXT::default().label_name(&c)) };
        }
    }

    fn end_label(&self) {
        if let Some(du) = &self.dev.exts.debug_utils {
            // SAFETY: matches a label begun on this command buffer.
            unsafe { du.cmd_end_debug_utils_label(self.cb) };
        }
    }

    fn barriers(&self, images: &[vk::ImageMemoryBarrier2<'_>], buffers: &[vk::BufferMemoryBarrier2<'_>], memory: &[vk::MemoryBarrier2<'_>]) {
        if images.is_empty() && buffers.is_empty() && memory.is_empty() {
            return;
        }
        let dep = vk::DependencyInfo::default().image_memory_barriers(images).buffer_memory_barriers(buffers).memory_barriers(memory);
        // SAFETY: recording, outside a render pass (callers check).
        unsafe { self.dev.raw.cmd_pipeline_barrier2(self.cb, &dep) };
    }

    /// Packs pending uploads into this slot's staging buffer and records their copies.
    fn apply_uploads(&mut self, uploads: Vec<Upload>) {
        let mut total = 0u64;
        for u in &uploads {
            let n = match u {
                Upload::Buffer { data, .. } | Upload::Texture { data, .. } => data.len() as u64,
            };
            total = total.next_multiple_of(16) + n;
        }
        let staging = {
            let mut st = self.dev.lock();
            let have = st.slots[self.slot].staging.as_ref().map(|s| s.bytes).unwrap_or(0);
            if have < total {
                if let Some(old) = st.slots[self.slot].staging.take() {
                    // The slot's previous frame (the last user) has finished.
                    self.dev.free_buffer(old);
                }
                let size = total.next_power_of_two().max(1 << 20);
                match self.dev.make_buffer(size, vk::BufferUsageFlags::TRANSFER_SRC, super::MemKind::Upload, &format!("ed_gpu staging (slot {})", self.slot)) {
                    Ok(b) => st.slots[self.slot].staging = Some(b),
                    Err(e) => {
                        drop(st);
                        self.fail(format!("uploads: staging allocation failed: {e}"));
                        return;
                    }
                }
            }
            let s = st.slots[self.slot].staging.as_ref().expect("staging allocated");
            (s.raw, s.ptr)
        };
        self.label("uploads");
        let mut offset = 0u64;
        for u in uploads {
            offset = offset.next_multiple_of(16);
            let data = match &u {
                Upload::Buffer { data, .. } | Upload::Texture { data, .. } => data,
            };
            // SAFETY: the staging buffer holds at least `total` bytes and is mapped; the slot's
            // previous frame has finished, so the GPU is not reading this range.
            unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), staging.1.add(offset as usize), data.len()) };
            let mut st = self.dev.lock();
            match u {
                Upload::Buffer { data, dst, offset: dst_off } => {
                    let Some(b) = st.buffers.get_mut(dst.index(), dst.generation()) else { continue };
                    let tracked = b.states.get(0, 0).unwrap_or(ResourceState::Undefined);
                    let r = b.states.expect(None, &[ResourceState::CopyDst, ResourceState::Undefined]);
                    b.states.set(None, ResourceState::CopyDst);
                    let raw = b.copies[0].raw;
                    drop(st);
                    self.check(r, "write_buffer (applied at frame start)");
                    let bb = buffer_barrier(raw, conv::access(tracked, Res::Buffer), conv::access(ResourceState::CopyDst, Res::Buffer));
                    self.barriers(&[], &[bb], &[]);
                    let region = vk::BufferCopy { src_offset: offset, dst_offset: dst_off, size: data.len() as u64 };
                    // SAFETY: the range was validated in write_buffer; both buffers are alive.
                    unsafe { self.dev.raw.cmd_copy_buffer(self.cb, staging.0, raw, &[region]) };
                    offset += data.len() as u64;
                }
                Upload::Texture { data, dst, sub } => {
                    let Some(t) = st.textures.get_mut(dst.index(), dst.generation()) else { continue };
                    let tracked = t.states.get(sub.mip, sub.layer).unwrap_or(ResourceState::Undefined);
                    let r = t.states.expect(Some(sub), &[ResourceState::CopyDst, ResourceState::Undefined]);
                    t.states.set(Some(sub), ResourceState::CopyDst);
                    let (image, aspect, res) = (t.image, t.aspect(), t.res());
                    let (w, h) = (mip_size(t.width, sub.mip), mip_size(t.height, sub.mip));
                    let d = if t.kind == TextureKind::D3 { mip_size(t.depth(), sub.mip) } else { 1 };
                    let layer = if t.kind == TextureKind::D3 { 0 } else { sub.layer };
                    drop(st);
                    self.check(r, "write_texture (applied at frame start)");
                    let old = if tracked == ResourceState::Undefined { vk::ImageLayout::UNDEFINED } else { conv::access(tracked, res).layout };
                    let ib = image_barrier(image, aspect, range(aspect, sub.mip, 1, layer, 1), conv::access(tracked, res), old, conv::access(ResourceState::CopyDst, res));
                    self.barriers(&[ib], &[], &[]);
                    let region = vk::BufferImageCopy {
                        buffer_offset: offset,
                        buffer_row_length: 0,
                        buffer_image_height: 0,
                        image_subresource: vk::ImageSubresourceLayers { aspect_mask: aspect, mip_level: sub.mip, base_array_layer: layer, layer_count: 1 },
                        image_offset: vk::Offset3D::default(),
                        image_extent: vk::Extent3D { width: w, height: h, depth: d },
                    };
                    // SAFETY: the data size was checked against this subresource in write_texture.
                    unsafe { self.dev.raw.cmd_copy_buffer_to_image(self.cb, staging.0, image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]) };
                    offset += data.len() as u64;
                }
            }
        }
        self.end_label();
    }

    fn texture_info(&mut self, id: TextureId, what: &str) -> Option<TexInfo> {
        let st = self.dev.lock();
        match st.textures.get(id.index(), id.generation()) {
            Some(t) => Some(TexInfo {
                image: t.image,
                aspect: t.aspect(),
                format: t.format,
                width: t.width,
                height: t.height,
                depth: t.depth(),
                kind: t.kind,
                mips: t.mips,
                layers: t.layers(),
                att_views: t.att_views.clone(),
            }),
            None => {
                drop(st);
                self.fail(format!("{what}: stale texture"));
                None
            }
        }
    }

    /// Checks a texture subresource state; returns false (and records) on a stale handle.
    /// On a state mismatch it records the error (returned by `end_frame`) and
    /// then repairs the layout with a barrier from the tracked state, so the command buffer
    /// stays valid Vulkan (Metal has no layouts and needs no repair).
    fn expect_tex(&mut self, id: TextureId, sub: Option<Subresource>, allowed: &[ResourceState], what: &str) -> bool {
        let (r, wrong) = {
            let st = self.dev.lock();
            match st.textures.get(id.index(), id.generation()) {
                Some(t) => {
                    let r = t.states.expect(sub, allowed);
                    let subs: Vec<Subresource> = match sub {
                        Some(s) => vec![s],
                        None => (0..t.states.layers()).flat_map(|layer| (0..t.states.mips()).map(move |mip| Subresource { mip, layer })).collect(),
                    };
                    let wrong: Vec<(Subresource, ResourceState)> =
                        if r.is_err() { subs.into_iter().filter_map(|s| t.states.get(s.mip, s.layer).filter(|c| !allowed.contains(c)).map(|c| (s, c))).collect() } else { Vec::new() };
                    (r, wrong)
                }
                None => {
                    drop(st);
                    self.fail(format!("{what}: stale texture"));
                    return false;
                }
            }
        };
        self.check(r, what);
        if !wrong.is_empty() && matches!(self.pass, Pass::None) {
            let fix: Vec<Transition> = wrong.into_iter().map(|(s, from)| Transition::Texture { texture: id, sub: Some(s), from, to: allowed[0] }).collect();
            self.transition(&fix);
        }
        true
    }

    /// Checks a buffer state and returns this frame's `VkBuffer` and the size.
    fn buffer(&mut self, id: BufferId, allowed: &[ResourceState], what: &str) -> Option<(vk::Buffer, u64)> {
        let (r, raw, size) = {
            let st = self.dev.lock();
            match st.buffers.get(id.index(), id.generation()) {
                Some(b) => (b.states.expect(None, allowed), b.copies[self.slot % b.copies.len()].raw, b.size),
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

    fn render(&mut self, what: &str) -> bool {
        if matches!(self.pass, Pass::Render { .. }) {
            true
        } else {
            self.fail(format!("{what}: needs an open render pass"));
            false
        }
    }

    fn compute(&mut self, what: &str) -> bool {
        if matches!(self.pass, Pass::Compute) {
            true
        } else {
            self.fail(format!("{what}: needs an open compute pass"));
            false
        }
    }

    fn outside(&mut self, what: &str) -> bool {
        if self.in_pass() {
            self.fail(format!("{what}: not allowed inside a pass"));
            false
        } else {
            true
        }
    }

    fn leave_pass(&mut self, what: &str) {
        if self.in_pass() {
            self.fail(format!("{what}: the previous pass was not ended"));
            self.close_pass();
        }
    }

    fn close_pass(&mut self) {
        match std::mem::replace(&mut self.pass, Pass::None) {
            // SAFETY: matches cmd_begin_rendering.
            Pass::Render { .. } => unsafe { self.dev.raw.cmd_end_rendering(self.cb) },
            Pass::Compute => {}
            Pass::None => return,
        }
        // Debug groups opened inside the pass are closed with it.
        while self.debug.last() == Some(&true) {
            self.debug.pop();
            self.end_label();
            self.fail("debug group opened inside a pass was not closed in it");
        }
        self.end_label();
        self.pipeline = false;
        self.index = false;
        self.compute_pipeline = false;
    }

    fn bind_common(&self, point: vk::PipelineBindPoint) {
        // SAFETY: recording; the set and layout belong to this device.
        unsafe {
            self.dev.raw.cmd_bind_descriptor_sets(self.cb, point, self.dev.layout, 0, &[self.set], &[]);
            self.dev.raw.cmd_push_constants(self.cb, self.dev.layout, all_stages(), 0, &ZERO);
        }
    }

    fn timestamp(&mut self) -> Option<u32> {
        if self.dev.caps.timers == TimerSource::None || self.next_query >= MAX_QUERIES {
            return None;
        }
        let q = self.next_query;
        self.next_query += 1;
        // SAFETY: recording, outside passes; the query was reset at frame start.
        unsafe { self.dev.raw.cmd_write_timestamp2(self.cb, vk::PipelineStageFlags2::ALL_COMMANDS, self.queries, q) };
        Some(q)
    }

    fn do_finish(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        if self.in_pass() {
            self.fail("end_frame: a pass is still open");
            self.close_pass();
        }
        if !self.timer_stack.is_empty() {
            self.fail("end_frame: timer scopes still open");
            self.timer_stack.clear();
        }
        while self.debug.pop().is_some() {
            self.end_label();
            self.fail("end_frame: debug groups still open");
        }
        let dev = self.dev;
        let mut st = dev.lock();
        let acquired = std::mem::take(&mut st.acquired);
        let mut present = Vec::new();
        let mut fix = Vec::new();
        for (surface, image_index, id) in acquired {
            if let Some(t) = st.textures.remove(id.index(), id.generation()) {
                let tracked = t.states.get(0, 0).unwrap_or(ResourceState::Undefined);
                if tracked != ResourceState::Present {
                    if dev.validation && self.error.is_none() {
                        self.error = Some(format!("end_frame: surface texture must be transitioned to Present (is in {tracked:?})"));
                    }
                    // Keep the swapchain image legal for presentation anyway.
                    let res = t.res();
                    let old = if tracked == ResourceState::Undefined { vk::ImageLayout::UNDEFINED } else { conv::access(tracked, res).layout };
                    fix.push(image_barrier(t.image, t.aspect(), range(t.aspect(), 0, 1, 0, 1), conv::access(tracked, res), old, conv::access(ResourceState::Present, res)));
                }
            }
            if let Some(s) = st.surfaces.get(surface.index(), surface.generation()) {
                present.push((s.swapchain, image_index, s.acquire_sems[self.slot], s.present_sems[image_index as usize], surface));
            }
        }
        drop(st);
        // Make every write of the frame visible to the host (readback buffers).
        let mb = [vk::MemoryBarrier2::default()
            .src_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
            .src_access_mask(vk::AccessFlags2::MEMORY_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags2::HOST)
            .dst_access_mask(vk::AccessFlags2::HOST_READ)];
        self.barriers(&fix, &[], &mb);
        // SAFETY: recording; every scope, pass and label is closed.
        let ended = unsafe { dev.raw.end_command_buffer(self.cb) };
        let waits: Vec<_> = present.iter().map(|p| vk::SemaphoreSubmitInfo::default().semaphore(p.2).stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)).collect();
        let mut signals = vec![vk::SemaphoreSubmitInfo::default().semaphore(dev.timeline).value(self.frame + 1).stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)];
        signals.extend(present.iter().map(|p| vk::SemaphoreSubmitInfo::default().semaphore(p.3).stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)));
        let cbs = [vk::CommandBufferSubmitInfo::default().command_buffer(self.cb)];
        let submitted = {
            let q = dev.queue();
            let r = ended.and_then(|_| {
                let submit = vk::SubmitInfo2::default().wait_semaphore_infos(&waits).command_buffer_infos(&cbs).signal_semaphore_infos(&signals);
                // SAFETY: the queue mutex is held; every handle in the submit is alive.
                unsafe { dev.raw.queue_submit2(*q, &[submit], vk::Fence::null()) }
            });
            match r {
                Ok(()) => {
                    let mut lost = Vec::new();
                    let mut suboptimal = Vec::new();
                    for (sc, idx, _, sem, surface) in &present {
                        let scs = [*sc];
                        let idxs = [*idx];
                        let ws = [*sem];
                        let info = vk::PresentInfoKHR::default().wait_semaphores(&ws).swapchains(&scs).image_indices(&idxs);
                        // SAFETY: the queue mutex is held; the image was acquired this frame.
                        match unsafe { dev.exts.swapchain.queue_present(*q, &info) } {
                            Ok(false) => {}
                            Ok(true) => suboptimal.push(*surface),
                            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::ERROR_SURFACE_LOST_KHR) => lost.push(*surface),
                            Err(e) => {
                                lost.push(*surface);
                                self.fail(format!("present: {e:?}"));
                            }
                        }
                    }
                    Ok((lost, suboptimal))
                }
                Err(e) => Err(e),
            }
        };
        let mut st = dev.lock();
        match submitted {
            Ok((lost, suboptimal)) => {
                for s in lost {
                    if let Some(s) = st.surfaces.get_mut(s.index(), s.generation()) {
                        s.lost = true;
                    }
                }
                for s in suboptimal {
                    if let Some(s) = st.surfaces.get_mut(s.index(), s.generation()) {
                        super::note_suboptimal(s, "present");
                    }
                }
            }
            Err(e) => {
                self.fail(format!("end_frame: submit failed: {e:?}"));
                // Signal the frame's timeline value from the host so pacing and deferred
                // destruction do not wait forever on a frame that never ran.
                // SAFETY: valid timeline; the value is larger than any signalled so far.
                let _ = unsafe { dev.raw.signal_semaphore(&vk::SemaphoreSignalInfo::default().semaphore(dev.timeline).value(self.frame + 1)) };
            }
        }
        st.in_flight.push_back(FrameRec { index: self.frame, slot: self.slot, scopes: std::mem::take(&mut self.scopes) });
        st.submitted = self.frame + 1;
        st.frame_open = false;
        dev.retire(&mut st);
        drop(st);
        let vk_errors = dev.take_errors();
        if !vk_errors.is_empty() {
            self.fail(format!("Vulkan validation: {}", vk_errors.join(" | ")));
        }
        match self.error.take() {
            Some(e) => Err(Error::Invalid(e)),
            None => Ok(()),
        }
    }
}

impl Drop for VulkanEncoder<'_> {
    fn drop(&mut self) {
        // An encoder dropped without end_frame still submits, so frame accounting stays whole.
        let _ = self.do_finish();
    }
}

impl CommandEncoder for VulkanEncoder<'_> {
    fn frame_index(&self) -> u64 {
        self.frame
    }

    fn transition(&mut self, transitions: &[Transition]) {
        if matches!(self.pass, Pass::Render { .. }) {
            return self.fail("transition: not allowed inside a render pass");
        }
        let mut errs = Vec::new();
        let mut images = Vec::new();
        let mut buffers = Vec::new();
        {
            let mut st = self.dev.lock();
            for t in transitions {
                match *t {
                    Transition::Buffer { buffer, from, to } => match st.buffers.get_mut(buffer.index(), buffer.generation()) {
                        Some(b) => {
                            let tracked = b.states.get(0, 0).unwrap_or(ResourceState::Undefined);
                            let allowed = usage_allows(&b.usage, to, false);
                            if !allowed {
                                errs.push((true, format!("transition: buffer usage does not allow {to:?}")));
                            }
                            if let Err(e) = b.states.transition(None, from, to) {
                                errs.push((true, format!("transition (buffer): {e}")));
                                b.states.set(None, to);
                            }
                            let (src, dst) = (conv::access(tracked, Res::Buffer), conv::access(to, Res::Buffer));
                            if allowed && !(tracked == to && src.src_access().is_empty()) {
                                let raw = b.copies[self.slot % b.copies.len()].raw;
                                buffers.push(buffer_barrier(raw, src, dst));
                            }
                        }
                        None => errs.push((false, "transition: stale buffer".to_string())),
                    },
                    Transition::Texture { texture, sub, from, to } => match st.textures.get_mut(texture.index(), texture.generation()) {
                        Some(tex) => {
                            let allowed = usage_allows(&tex.usage, to, tex.surface);
                            if !allowed {
                                errs.push((true, format!("transition: texture usage does not allow {to:?}")));
                            }
                            // Subresources covered, with their tracked states before the move.
                            let subs: Vec<(u32, u32)> = match sub {
                                Some(s) if s.mip < tex.mips && s.layer < tex.layers() => vec![(s.mip, s.layer)],
                                Some(_) => Vec::new(),
                                None => (0..tex.layers()).flat_map(|l| (0..tex.mips).map(move |m| (m, l))).collect(),
                            };
                            let before: Vec<ResourceState> = subs.iter().map(|&(m, l)| tex.states.get(m, l).unwrap_or(ResourceState::Undefined)).collect();
                            if let Err(e) = tex.states.transition(sub, from, to) {
                                errs.push((true, format!("transition (texture): {e}")));
                                tex.states.set(sub, to);
                            }
                            if !allowed || to == ResourceState::Undefined {
                                continue;
                            }
                            let (res, aspect, image) = (tex.res(), tex.aspect(), tex.image);
                            let dst = conv::access(to, res);
                            let mut push = |tracked: ResourceState, r: vk::ImageSubresourceRange| {
                                let src = conv::access(tracked, res);
                                let old = if from == ResourceState::Undefined || tracked == ResourceState::Undefined { vk::ImageLayout::UNDEFINED } else { src.layout };
                                if tracked == to && old == dst.layout && src.src_access().is_empty() {
                                    return;
                                }
                                images.push(image_barrier(image, aspect, r, src, old, dst));
                            };
                            let uniform = before.windows(2).all(|w| w[0] == w[1]);
                            if uniform && sub.is_none() && !before.is_empty() {
                                push(before[0], range(aspect, 0, tex.mips, 0, tex.layers()));
                            } else {
                                for (&(m, l), &b) in subs.iter().zip(&before) {
                                    push(b, range(aspect, m, 1, l, 1));
                                }
                            }
                        }
                        None => errs.push((false, "transition: stale texture".to_string())),
                    },
                }
            }
        }
        for (is_state, e) in errs {
            if is_state {
                self.check(Err(e), "transition");
            } else {
                self.fail(e);
            }
        }
        self.barriers(&images, &buffers, &[]);
    }

    fn begin_render_pass(&mut self, desc: &RenderPassDesc<'_>) {
        self.leave_pass("begin_render_pass");
        if desc.colors.len() > 8 {
            return self.fail("begin_render_pass: more than 8 colour attachments");
        }
        let mut colors = Vec::new();
        let mut color_infos = Vec::new();
        let mut extent: Option<vk::Extent2D> = None;
        let mut check_extent = |e: &mut Self, w: u32, h: u32| -> bool {
            let x = vk::Extent2D { width: w, height: h };
            match extent {
                Some(p) if p != x => {
                    e.fail("begin_render_pass: attachments differ in size");
                    false
                }
                _ => {
                    extent = Some(x);
                    true
                }
            }
        };
        for a in desc.colors {
            if !self.expect_tex(a.texture, Some(a.sub), &[ResourceState::RenderTarget], "begin_render_pass (colour)") {
                return;
            }
            let Some(t) = self.texture_info(a.texture, "begin_render_pass") else { return };
            if t.kind == TextureKind::D3 {
                return self.fail("begin_render_pass: 3D render targets are not supported on Vulkan");
            }
            let Some(&view) = t.att_views.get((a.sub.layer * t.mips + a.sub.mip) as usize) else {
                return self.fail("begin_render_pass: colour attachment has no render-target view (usage) or subresource out of range");
            };
            if !check_extent(self, mip_size(t.width, a.sub.mip), mip_size(t.height, a.sub.mip)) {
                return;
            }
            colors.push(t.format);
            let mut info = vk::RenderingAttachmentInfo::default()
                .image_view(view)
                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(conv::load(a.load))
                .store_op(conv::store(a.store));
            if let LoadOp::Clear(c) = a.load {
                info = info.clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: c } });
            }
            color_infos.push(info);
        }
        let mut depth = None;
        let mut depth_info = None;
        if let Some(a) = &desc.depth {
            let is_write = {
                let st = self.dev.lock();
                st.textures.get(a.texture.index(), a.texture.generation()).map(|t| t.states.expect(Some(a.sub), &[ResourceState::DepthWrite]).is_ok())
            };
            if !self.expect_tex(a.texture, Some(a.sub), &[ResourceState::DepthWrite, ResourceState::DepthRead], "begin_render_pass (depth)") {
                return;
            }
            let Some(t) = self.texture_info(a.texture, "begin_render_pass") else { return };
            let Some(&view) = t.att_views.get((a.sub.layer * t.mips + a.sub.mip) as usize) else {
                return self.fail("begin_render_pass: depth attachment has no depth view or subresource out of range");
            };
            if !check_extent(self, mip_size(t.width, a.sub.mip), mip_size(t.height, a.sub.mip)) {
                return;
            }
            let state = if is_write == Some(true) { ResourceState::DepthWrite } else { ResourceState::DepthRead };
            depth = Some((t.format, state));
            let mut info = vk::RenderingAttachmentInfo::default()
                .image_view(view)
                .image_layout(conv::access(state, Res::Texture { sampled_layout: vk::ImageLayout::UNDEFINED, surface: false }).layout)
                .load_op(conv::load(a.load))
                .store_op(conv::store(a.store));
            if let LoadOp::Clear(c) = a.load {
                info = info.clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: c[0], stencil: 0 } });
            }
            depth_info = Some(info);
        }
        let Some(extent) = extent else { return self.fail("begin_render_pass: no attachments") };
        self.label(desc.label);
        let area = vk::Rect2D { offset: vk::Offset2D::default(), extent };
        let mut ri = vk::RenderingInfo::default().render_area(area).layer_count(1).color_attachments(&color_infos);
        if let Some(d) = &depth_info {
            ri = ri.depth_attachment(d);
        }
        // SAFETY: recording outside any pass; attachments validated above and alive (handles held
        // by the device until frames that use them finish).
        unsafe {
            self.dev.raw.cmd_begin_rendering(self.cb, &ri);
        }
        self.bind_common(vk::PipelineBindPoint::GRAPHICS);
        self.pass = Pass::Render { colors, depth };
        self.set_viewport(0.0, 0.0, extent.width as f32, extent.height as f32);
        self.set_scissor(0, 0, extent.width, extent.height);
    }

    fn end_render_pass(&mut self) {
        if !matches!(self.pass, Pass::Render { .. }) {
            return self.fail("end_render_pass: no render pass is open");
        }
        self.close_pass();
    }

    fn set_viewport(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if self.render("set_viewport") {
            // Negative height flips Y so clip space is Y-up as on Metal and D3D12.
            let vp = vk::Viewport { x, y: y + h, width: w, height: -h, min_depth: 0.0, max_depth: 1.0 };
            // SAFETY: recording inside a render pass; the viewport is dynamic state.
            unsafe { self.dev.raw.cmd_set_viewport(self.cb, 0, &[vp]) };
        }
    }

    fn set_scissor(&mut self, x: u32, y: u32, w: u32, h: u32) {
        if self.render("set_scissor") {
            let r = vk::Rect2D { offset: vk::Offset2D { x: x as i32, y: y as i32 }, extent: vk::Extent2D { width: w, height: h } };
            // SAFETY: recording inside a render pass; the scissor is dynamic state.
            unsafe { self.dev.raw.cmd_set_scissor(self.cb, 0, &[r]) };
        }
    }

    fn set_render_pipeline(&mut self, pipeline: RenderPipelineId) {
        if !self.render("set_render_pipeline") {
            return;
        }
        let p = {
            let st = self.dev.lock();
            st.rpipes.get(pipeline.index(), pipeline.generation()).map(|p| (p.raw, p.colors.clone(), p.depth))
        };
        let Some((raw, colors, depth)) = p else { return self.fail("set_render_pipeline: stale pipeline") };
        let Pass::Render { colors: pc, depth: pd, .. } = &self.pass else { return };
        if colors != *pc || depth.map(|d| d.0) != pd.map(|d| d.0) {
            let msg = format!("set_render_pipeline: pipeline targets {colors:?}/{depth:?} do not match the pass {pc:?}/{pd:?}");
            return self.fail(msg);
        }
        if let (Some((_, true)), Some((_, ResourceState::DepthRead))) = (depth, *pd) {
            self.check(Err("pipeline writes depth but the depth attachment is in DepthRead".into()), "set_render_pipeline");
        }
        // SAFETY: recording inside a render pass whose formats match the pipeline.
        unsafe { self.dev.raw.cmd_bind_pipeline(self.cb, vk::PipelineBindPoint::GRAPHICS, raw) };
        self.pipeline = true;
    }

    fn set_index_buffer(&mut self, buffer: BufferId, offset: u64) {
        if !self.render("set_index_buffer") {
            return;
        }
        if !offset.is_multiple_of(4) {
            return self.fail("set_index_buffer: offset must be a multiple of 4");
        }
        if let Some((raw, _)) = self.buffer(buffer, &[ResourceState::ShaderRead], "set_index_buffer") {
            // SAFETY: recording; the buffer has INDEX_BUFFER usage (shader_read buffers get it).
            unsafe { self.dev.raw.cmd_bind_index_buffer(self.cb, raw, offset, vk::IndexType::UINT32) };
            self.index = true;
        }
    }

    fn set_draw_constants(&mut self, data: &[u8; DRAW_CONSTANTS_BYTES]) {
        if self.render("set_draw_constants") {
            // SAFETY: recording; 64 bytes within the layout's push range.
            unsafe { self.dev.raw.cmd_push_constants(self.cb, self.dev.layout, all_stages(), 0, data) };
        }
    }

    fn draw(&mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>) {
        if !self.render("draw") {
            return;
        }
        if !self.pipeline {
            return self.fail("draw: no pipeline set");
        }
        if vertices.is_empty() || instances.is_empty() {
            return;
        }
        // SAFETY: recording inside a render pass with a pipeline bound.
        unsafe { self.dev.raw.cmd_draw(self.cb, vertices.len() as u32, instances.len() as u32, vertices.start, instances.start) };
    }

    fn draw_indexed(&mut self, indices: std::ops::Range<u32>, base_vertex: i32, instances: std::ops::Range<u32>) {
        if !self.render("draw_indexed") {
            return;
        }
        if !self.pipeline {
            return self.fail("draw_indexed: no pipeline set");
        }
        if !self.index {
            return self.fail("draw_indexed: no index buffer set");
        }
        if indices.is_empty() || instances.is_empty() {
            return;
        }
        // SAFETY: recording inside a render pass with a pipeline and index buffer bound.
        unsafe { self.dev.raw.cmd_draw_indexed(self.cb, indices.len() as u32, instances.len() as u32, indices.start, base_vertex, instances.start) };
    }

    fn draw_indexed_indirect_multi(&mut self, args: BufferId, args_offset: u64, max_draws: u32, count: Option<(BufferId, u64)>) {
        if !self.render("draw_indexed_indirect_multi") {
            return;
        }
        if !self.pipeline {
            return self.fail("draw_indexed_indirect_multi: no pipeline set");
        }
        if !self.index {
            return self.fail("draw_indexed_indirect_multi: no index buffer set");
        }
        let Some((ab, size)) = self.buffer(args, &[ResourceState::IndirectArgs], "draw_indexed_indirect_multi (args)") else { return };
        let count = match count {
            Some((cbuf, off)) => {
                let Some((c, csize)) = self.buffer(cbuf, &[ResourceState::IndirectArgs], "draw_indexed_indirect_multi (count)") else { return };
                if !off.is_multiple_of(4) || off + 4 > csize {
                    return self.fail("draw_indexed_indirect_multi: count outside the buffer or unaligned");
                }
                Some((c, off))
            }
            None => None,
        };
        let stride = std::mem::size_of::<DrawIndexedArgs>() as u32;
        if !args_offset.is_multiple_of(4) || args_offset + stride as u64 * max_draws as u64 > size {
            return self.fail("draw_indexed_indirect_multi: args range outside the buffer or unaligned");
        }
        if max_draws == 0 {
            return;
        }
        // SAFETY: recording inside a render pass with pipeline and index buffer bound; ranges
        // checked above; buffers have INDIRECT_BUFFER usage (indirect_args).
        unsafe {
            match count {
                Some((c, off)) if self.dev.indirect_count => self.dev.raw.cmd_draw_indexed_indirect_count(self.cb, ab, args_offset, c, off, max_draws, stride),
                _ if self.dev.multi_draw => self.dev.raw.cmd_draw_indexed_indirect(self.cb, ab, args_offset, max_draws, stride),
                _ => {
                    for i in 0..max_draws as u64 {
                        self.dev.raw.cmd_draw_indexed_indirect(self.cb, ab, args_offset + i * stride as u64, 1, stride);
                    }
                }
            }
        }
    }

    fn begin_compute_pass(&mut self, label: &str) {
        self.leave_pass("begin_compute_pass");
        self.label(label);
        self.bind_common(vk::PipelineBindPoint::COMPUTE);
        self.pass = Pass::Compute;
    }

    fn end_compute_pass(&mut self) {
        if !matches!(self.pass, Pass::Compute) {
            return self.fail("end_compute_pass: no compute pass is open");
        }
        self.close_pass();
    }

    fn set_compute_pipeline(&mut self, pipeline: ComputePipelineId) {
        if !self.compute("set_compute_pipeline") {
            return;
        }
        let raw = self.dev.lock().cpipes.get(pipeline.index(), pipeline.generation()).map(|p| p.raw);
        let Some(raw) = raw else { return self.fail("set_compute_pipeline: stale pipeline") };
        // SAFETY: recording.
        unsafe { self.dev.raw.cmd_bind_pipeline(self.cb, vk::PipelineBindPoint::COMPUTE, raw) };
        self.compute_pipeline = true;
    }

    fn set_compute_constants(&mut self, data: &[u8; DRAW_CONSTANTS_BYTES]) {
        if self.compute("set_compute_constants") {
            // SAFETY: recording; 64 bytes within the push range.
            unsafe { self.dev.raw.cmd_push_constants(self.cb, self.dev.layout, all_stages(), 0, data) };
        }
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        if !self.compute("dispatch") {
            return;
        }
        if !self.compute_pipeline {
            return self.fail("dispatch: no pipeline set");
        }
        if x == 0 || y == 0 || z == 0 {
            return;
        }
        // SAFETY: recording with a compute pipeline bound.
        unsafe { self.dev.raw.cmd_dispatch(self.cb, x, y, z) };
    }

    fn dispatch_indirect(&mut self, args: BufferId, offset: u64) {
        if !self.compute("dispatch_indirect") {
            return;
        }
        if !self.compute_pipeline {
            return self.fail("dispatch_indirect: no pipeline set");
        }
        let Some((ab, size)) = self.buffer(args, &[ResourceState::IndirectArgs], "dispatch_indirect") else { return };
        if !offset.is_multiple_of(4) || offset + 12 > size {
            return self.fail("dispatch_indirect: args outside the buffer or unaligned");
        }
        // SAFETY: recording with a compute pipeline bound; range checked.
        unsafe { self.dev.raw.cmd_dispatch_indirect(self.cb, ab, offset) };
    }

    fn copy_buffer(&mut self, src: BufferId, src_offset: u64, dst: BufferId, dst_offset: u64, size: u64) {
        if !self.outside("copy_buffer") {
            return;
        }
        let Some((s, ss)) = self.buffer(src, &[ResourceState::CopySrc], "copy_buffer (src)") else { return };
        let Some((d, ds)) = self.buffer(dst, &[ResourceState::CopyDst], "copy_buffer (dst)") else { return };
        if src_offset + size > ss || dst_offset + size > ds {
            return self.fail("copy_buffer: range outside a buffer");
        }
        if size == 0 {
            return;
        }
        // SAFETY: recording outside passes; ranges checked.
        unsafe { self.dev.raw.cmd_copy_buffer(self.cb, s, d, &[vk::BufferCopy { src_offset, dst_offset, size }]) };
    }

    fn copy_texture(&mut self, src: TextureId, src_sub: Subresource, dst: TextureId, dst_sub: Subresource) {
        if !self.outside("copy_texture") {
            return;
        }
        if !self.expect_tex(src, Some(src_sub), &[ResourceState::CopySrc], "copy_texture (src)") || !self.expect_tex(dst, Some(dst_sub), &[ResourceState::CopyDst], "copy_texture (dst)") {
            return;
        }
        let Some(s) = self.texture_info(src, "copy_texture") else { return };
        let Some(d) = self.texture_info(dst, "copy_texture") else { return };
        let size = (mip_size(s.width, src_sub.mip), mip_size(s.height, src_sub.mip));
        if s.format != d.format || size != (mip_size(d.width, dst_sub.mip), mip_size(d.height, dst_sub.mip)) {
            return self.fail("copy_texture: formats or subresource sizes differ");
        }
        let depth = if s.kind == TextureKind::D3 { mip_size(s.depth, src_sub.mip) } else { 1 };
        let layers = |t: &TexInfo, sub: Subresource| vk::ImageSubresourceLayers {
            aspect_mask: t.aspect,
            mip_level: sub.mip,
            base_array_layer: if t.kind == TextureKind::D3 { 0 } else { sub.layer },
            layer_count: 1,
        };
        let region = vk::ImageCopy {
            src_subresource: layers(&s, src_sub),
            src_offset: vk::Offset3D::default(),
            dst_subresource: layers(&d, dst_sub),
            dst_offset: vk::Offset3D { x: 0, y: 0, z: if d.kind == TextureKind::D3 { dst_sub.layer as i32 } else { 0 } },
            extent: vk::Extent3D { width: size.0, height: size.1, depth },
        };
        // SAFETY: recording outside passes; subresources validated; layouts match the states.
        unsafe { self.dev.raw.cmd_copy_image(self.cb, s.image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, d.image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]) };
    }

    fn copy_texture_to_buffer(&mut self, src: TextureId, sub: Subresource, dst: BufferId, dst_offset: u64, bytes_per_row: u32) {
        if !self.outside("copy_texture_to_buffer") {
            return;
        }
        if !self.expect_tex(src, Some(sub), &[ResourceState::CopySrc], "copy_texture_to_buffer (src)") {
            return;
        }
        let Some(t) = self.texture_info(src, "copy_texture_to_buffer") else { return };
        let Some((b, bs)) = self.buffer(dst, &[ResourceState::CopyDst], "copy_texture_to_buffer (dst)") else { return };
        let f = t.format;
        let (w, h) = (mip_size(t.width, sub.mip), mip_size(t.height, sub.mip));
        let depth = if t.kind == TextureKind::D3 { mip_size(t.depth, sub.mip) } else { 1 };
        if !bytes_per_row.is_multiple_of(COPY_ROW_PITCH_ALIGN) || bytes_per_row < f.packed_row_bytes(w) {
            return self.fail(format!("copy_texture_to_buffer: bytes_per_row {bytes_per_row} must be a multiple of {COPY_ROW_PITCH_ALIGN} and at least {}", f.packed_row_bytes(w)));
        }
        let (bw, bh, bb) = f.block();
        if !dst_offset.is_multiple_of(4) || !dst_offset.is_multiple_of(bb as u64) || !bytes_per_row.is_multiple_of(bb) {
            return self.fail("copy_texture_to_buffer: dst_offset must be a multiple of 4 and of the block size");
        }
        let image = bytes_per_row as u64 * f.block_rows(h) as u64;
        if dst_offset + image * depth as u64 > bs {
            return self.fail("copy_texture_to_buffer: buffer too small");
        }
        let region = vk::BufferImageCopy {
            buffer_offset: dst_offset,
            buffer_row_length: bytes_per_row / bb * bw,
            buffer_image_height: f.block_rows(h) * bh,
            image_subresource: vk::ImageSubresourceLayers {
                aspect_mask: t.aspect,
                mip_level: sub.mip,
                base_array_layer: if t.kind == TextureKind::D3 { 0 } else { sub.layer },
                layer_count: 1,
            },
            image_offset: vk::Offset3D::default(),
            image_extent: vk::Extent3D { width: w, height: h, depth },
        };
        // SAFETY: recording outside passes; sizes and the destination range validated.
        unsafe { self.dev.raw.cmd_copy_image_to_buffer(self.cb, t.image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, b, &[region]) };
    }

    fn generate_mips(&mut self, texture: TextureId) {
        if !self.outside("generate_mips") {
            return;
        }
        if !self.expect_tex(texture, None, &[ResourceState::CopyDst], "generate_mips") {
            return;
        }
        let Some(t) = self.texture_info(texture, "generate_mips") else { return };
        if t.mips < 2 {
            return;
        }
        let need = vk::FormatFeatureFlags::BLIT_SRC | vk::FormatFeatureFlags::BLIT_DST | vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR;
        if !self.dev.format_supports(conv::format(t.format), need) {
            return self.fail(format!("generate_mips: format {:?} cannot be blitted with linear filtering on this device", t.format));
        }
        let transfer = |access: vk::AccessFlags2, layout| Access { stages: vk::PipelineStageFlags2::ALL_TRANSFER, access, layout };
        let dst = transfer(vk::AccessFlags2::TRANSFER_WRITE, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
        let src = transfer(vk::AccessFlags2::TRANSFER_READ, vk::ImageLayout::TRANSFER_SRC_OPTIMAL);
        let to_dst = Access { access: vk::AccessFlags2::TRANSFER_WRITE, ..dst };
        for m in 1..t.mips {
            let prev = range(t.aspect, m - 1, 1, 0, t.layers);
            // The previous mip (written by an upload or the last blit) becomes the source; the
            // target mip's earlier writes (uploads) are ordered before the blit overwrites it.
            self.barriers(
                &[image_barrier(t.image, t.aspect, prev, dst, dst.layout, src), image_barrier(t.image, t.aspect, range(t.aspect, m, 1, 0, t.layers), dst, dst.layout, to_dst)],
                &[],
                &[],
            );
            let off = |mip: u32| vk::Offset3D {
                x: mip_size(t.width, mip) as i32,
                y: mip_size(t.height, mip) as i32,
                z: if t.kind == TextureKind::D3 { mip_size(t.depth, mip) as i32 } else { 1 },
            };
            let sl = |mip| vk::ImageSubresourceLayers { aspect_mask: t.aspect, mip_level: mip, base_array_layer: 0, layer_count: t.layers };
            let blit = vk::ImageBlit { src_subresource: sl(m - 1), src_offsets: [vk::Offset3D::default(), off(m - 1)], dst_subresource: sl(m), dst_offsets: [vk::Offset3D::default(), off(m)] };
            // SAFETY: recording outside passes; mip m-1 is TRANSFER_SRC, mip m TRANSFER_DST.
            unsafe {
                self.dev.raw.cmd_blit_image(self.cb, t.image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, t.image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[blit], vk::Filter::LINEAR)
            };
            // Back to CopyDst, as the contract says every mip ends in.
            let back = Access { stages: vk::PipelineStageFlags2::ALL_TRANSFER, access: vk::AccessFlags2::TRANSFER_WRITE, layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL };
            self.barriers(&[image_barrier(t.image, t.aspect, prev, src, src.layout, back)], &[], &[]);
        }
    }

    fn begin_timer(&mut self, label: &'static str) {
        if self.in_pass() {
            return self.fail("begin_timer: timer scopes go around passes, not inside them");
        }
        let q = self.timestamp();
        self.timer_stack.push((label, q));
    }

    fn end_timer(&mut self) {
        if self.in_pass() {
            return self.fail("end_timer: timer scopes go around passes, not inside them");
        }
        let Some((label, begin)) = self.timer_stack.pop() else { return self.fail("end_timer without begin_timer") };
        let depth = self.timer_stack.len() as u32;
        let end = self.timestamp();
        if let (Some(begin), Some(end)) = (begin, end) {
            self.scopes.push(Scope { label, depth, begin, end });
        }
    }

    fn debug_group(&mut self, label: &str) {
        self.label(label);
        self.debug.push(self.in_pass());
    }

    fn end_debug_group(&mut self) {
        match self.debug.pop() {
            None => self.fail("end_debug_group without debug_group"),
            Some(true) if !self.in_pass() => self.fail("end_debug_group: the group was opened inside a pass that has ended"),
            Some(false) if self.in_pass() => {
                self.debug.push(false);
                self.fail("end_debug_group: a group opened outside a pass must close outside it");
            }
            Some(_) => self.end_label(),
        }
    }

    fn finish(mut self: Box<Self>) -> Result<()> {
        self.do_finish()
    }
}
