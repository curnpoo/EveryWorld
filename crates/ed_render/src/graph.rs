//! Frame graph (P1-D3).
//!
//! A frame is described as passes that declare which resources they read and write, and in
//! which [`ResourceState`]. The graph then:
//!
//! - orders the passes from those declarations (not from the order they were added),
//! - culls passes whose results nobody uses,
//! - assigns transient textures to physical textures from a [`TransientPool`], aliasing
//!   transients whose lifetimes do not overlap (no per-frame GPU allocation after warm-up),
//! - emits every state transition between passes (`docs/contracts/rhi.md` section 6: a
//!   transition outside a pass is the only synchronisation),
//! - wraps every pass in a GPU timer scope and a debug group, and the whole frame in a
//!   [`FRAME_SCOPE`] timer.
//!
//! Dependency rule: a pass that writes a resource runs after the passes declared earlier that
//! also write it (so a clear pass and a later `LoadOp::Load` pass keep their order); a pass
//! that reads a resource runs after *every* pass that writes it. A resource therefore has one
//! final value per frame that all its readers see. Ping-pong patterns use two resources.
//!
//! Roots (never culled): passes marked [`PassBuilder::side_effect`] and passes that write an
//! imported resource. Everything else survives only if a root needs it.
//!
//! [`FrameGraph::compile`] is pure (no GPU) and is what the unit tests check;
//! [`FrameGraph::execute`] records the compiled plan into a [`CommandEncoder`].

use ed_gpu::{
    Attachment, BindlessTexture, BufferId, CommandEncoder, Device, Format, LoadOp, RenderPassDesc, ResourceState, StoreOp, Subresource,
    TextureDesc, TextureId, TextureKind, Timing, Transition, Usage,
};

/// Label of the timer scope around the whole frame (depth 0); passes are at depth 1.
pub const FRAME_SCOPE: &str = "frame";

/// Frames a pooled transient may go unused before it is destroyed. Larger than any
/// frames-in-flight count, so a resize does not keep two sizes alive for long.
pub const EVICT_AFTER_FRAMES: u64 = 4;

/// A resource declared in one frame graph.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct Res(u32);

impl Res {
    /// Index into [`Plan::physical`] and [`Plan::final_states`].
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Shape of a 2D texture (transient or imported).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TextureInfo {
    pub format: Format,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug)]
enum ResKind {
    Transient(TextureInfo),
    ImportTexture { id: TextureId, info: TextureInfo, initial: ResourceState, final_state: Option<ResourceState> },
    ImportBuffer { id: ed_gpu::BufferId, initial: ResourceState, final_state: Option<ResourceState> },
}

#[derive(Clone, Copy, Debug)]
struct ResDecl {
    name: &'static str,
    kind: ResKind,
}

/// How the graph opens the pass.
#[derive(Clone, Debug)]
pub enum PassKind {
    /// The graph begins and ends a render pass with the declared attachments and sets the
    /// viewport to the first attachment's size.
    Raster,
    /// The graph begins and ends a compute pass.
    Compute,
    /// The graph emits transitions, timer and debug group; the callback records anything
    /// else itself (copies, several passes, ...). It must leave every declared resource in
    /// the state it declared.
    Manual,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Access {
    res: Res,
    state: ResourceState,
    write: bool,
}

#[derive(Clone, Copy, Debug)]
struct ColorAtt {
    res: Res,
    load: LoadOp,
    store: StoreOp,
}

type Exec<'a> = Box<dyn FnOnce(&mut PassCtx<'_>) + 'a>;

struct PassDecl<'a> {
    name: &'static str,
    kind: PassKind,
    accesses: Vec<Access>,
    colors: Vec<ColorAtt>,
    depth: Option<ColorAtt>,
    side_effect: bool,
    exec: Option<Exec<'a>>,
}

/// One transition in a compiled plan.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PlannedTransition {
    pub res: Res,
    pub from: ResourceState,
    pub to: ResourceState,
}

/// One physical transient texture used by a plan.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PhysicalSlot {
    pub info: TextureInfo,
    pub usage: Usage,
}

/// The result of [`FrameGraph::compile`].
#[derive(Clone, Debug, Default)]
pub struct Plan {
    /// Indices (in declaration order) of the passes to run, in run order.
    pub order: Vec<usize>,
    /// Indices of passes that were culled.
    pub culled: Vec<usize>,
    /// Transitions to record before `order[i]`.
    pub before: Vec<Vec<PlannedTransition>>,
    /// Transitions after the last pass (imports with a final state).
    pub after: Vec<PlannedTransition>,
    /// Physical slot of each transient resource used by a running pass (index = `Res`).
    pub physical: Vec<Option<usize>>,
    pub slots: Vec<PhysicalSlot>,
    /// State of every resource at the end of the frame (index = `Res`).
    pub final_states: Vec<ResourceState>,
}

impl Plan {
    /// Total number of transitions the plan records.
    pub fn transition_count(&self) -> usize {
        self.before.iter().map(Vec::len).sum::<usize>() + self.after.len()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum GraphError {
    /// The declared reads and writes form a cycle; the names of the passes on it.
    Cycle(Vec<&'static str>),
    /// One pass declared two different states for the same resource.
    ConflictingAccess { pass: &'static str, res: &'static str },
    /// A transient is read but no pass writes it.
    ReadBeforeWrite { pass: &'static str, res: &'static str },
    /// A handle from another graph.
    UnknownResource,
    /// An attachment or `bindless`/`buffer` lookup on the wrong kind of resource.
    WrongKind { res: &'static str, why: &'static str },
    Gpu(String),
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::Cycle(p) => write!(f, "frame graph cycle through passes {p:?}"),
            GraphError::ConflictingAccess { pass, res } => write!(f, "pass {pass} declares two states for {res}"),
            GraphError::ReadBeforeWrite { pass, res } => write!(f, "pass {pass} reads transient {res}, which no pass writes"),
            GraphError::UnknownResource => write!(f, "resource handle from another graph"),
            GraphError::WrongKind { res, why } => write!(f, "{res}: {why}"),
            GraphError::Gpu(e) => write!(f, "gpu: {e}"),
        }
    }
}
impl std::error::Error for GraphError {}

impl From<ed_gpu::Error> for GraphError {
    fn from(e: ed_gpu::Error) -> Self {
        GraphError::Gpu(e.to_string())
    }
}

/// Builds the passes of one frame. Build a new one per frame (it only holds small vectors).
#[derive(Default)]
pub struct FrameGraph<'a> {
    resources: Vec<ResDecl>,
    passes: Vec<PassDecl<'a>>,
}

/// Declares one pass; returned by [`FrameGraph::add_pass`].
pub struct PassBuilder<'g, 'a> {
    graph: &'g mut FrameGraph<'a>,
    pass: usize,
}

impl<'a> PassBuilder<'_, 'a> {
    fn p(&mut self) -> &mut PassDecl<'a> {
        &mut self.graph.passes[self.pass]
    }
    /// The pass reads `res` in `state` (e.g. `ShaderRead`, `CopySrc`, `DepthRead`).
    pub fn read(mut self, res: Res, state: ResourceState) -> Self {
        self.p().accesses.push(Access { res, state, write: false });
        self
    }
    /// The pass writes `res` in `state` (e.g. `ShaderWrite`, `CopyDst`).
    pub fn write(mut self, res: Res, state: ResourceState) -> Self {
        self.p().accesses.push(Access { res, state, write: true });
        self
    }
    /// A colour attachment (a write in `RenderTarget`). Raster passes only.
    pub fn color(mut self, res: Res, load: LoadOp, store: StoreOp) -> Self {
        self.p().accesses.push(Access { res, state: ResourceState::RenderTarget, write: true });
        self.p().colors.push(ColorAtt { res, load, store });
        self
    }
    /// The depth attachment: written in `DepthWrite`, or only tested in `DepthRead`.
    pub fn depth(mut self, res: Res, load: LoadOp, store: StoreOp, write: bool) -> Self {
        let state = if write { ResourceState::DepthWrite } else { ResourceState::DepthRead };
        self.p().accesses.push(Access { res, state, write });
        self.p().depth = Some(ColorAtt { res, load, store });
        self
    }
    /// Never cull this pass.
    pub fn side_effect(mut self) -> Self {
        self.p().side_effect = true;
        self
    }
    /// The commands of the pass.
    pub fn exec(mut self, f: impl FnOnce(&mut PassCtx<'_>) + 'a) {
        self.p().exec = Some(Box::new(f));
    }
    /// Finish the declaration without commands (tests, reserved passes).
    pub fn done(self) {}
}

impl<'a> FrameGraph<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    fn add_res(&mut self, name: &'static str, kind: ResKind) -> Res {
        self.resources.push(ResDecl { name, kind });
        Res(self.resources.len() as u32 - 1)
    }

    /// A texture that lives only inside this frame. Its contents start undefined; the
    /// physical texture comes from the pool and may be shared with other transients.
    pub fn create_texture(&mut self, name: &'static str, info: TextureInfo) -> Res {
        self.add_res(name, ResKind::Transient(info))
    }

    /// A texture owned outside the graph, currently in `initial`. When `final_state` is set
    /// the graph transitions it there after the last pass.
    pub fn import_texture(
        &mut self,
        name: &'static str,
        id: TextureId,
        info: TextureInfo,
        initial: ResourceState,
        final_state: Option<ResourceState>,
    ) -> Res {
        self.add_res(name, ResKind::ImportTexture { id, info, initial, final_state })
    }

    /// A buffer owned outside the graph, currently in `initial`.
    pub fn import_buffer(&mut self, name: &'static str, id: BufferId, initial: ResourceState, final_state: Option<ResourceState>) -> Res {
        self.add_res(name, ResKind::ImportBuffer { id, initial, final_state })
    }

    /// Declare a pass. `name` is also its timer label.
    pub fn add_pass(&mut self, name: &'static str, kind: PassKind) -> PassBuilder<'_, 'a> {
        self.passes.push(PassDecl { name, kind, accesses: Vec::new(), colors: Vec::new(), depth: None, side_effect: false, exec: None });
        let pass = self.passes.len() - 1;
        PassBuilder { graph: self, pass }
    }

    pub fn pass_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.passes.iter().map(|p| p.name)
    }

    fn res(&self, r: Res) -> Result<&ResDecl, GraphError> {
        self.resources.get(r.0 as usize).ok_or(GraphError::UnknownResource)
    }

    /// Orders, culls, aliases and plans transitions. Pure: touches no GPU.
    pub fn compile(&self) -> Result<Plan, GraphError> {
        let np = self.passes.len();
        let nr = self.resources.len();
        // Validate accesses.
        for p in &self.passes {
            for (i, a) in p.accesses.iter().enumerate() {
                let decl = self.res(a.res)?;
                if p.accesses[..i].iter().any(|b| b.res == a.res && b.state != a.state) {
                    return Err(GraphError::ConflictingAccess { pass: p.name, res: decl.name });
                }
                match decl.kind {
                    ResKind::ImportBuffer { .. } if p.colors.iter().chain(p.depth.iter()).any(|c| c.res == a.res) => {
                        return Err(GraphError::WrongKind { res: decl.name, why: "a buffer cannot be an attachment" });
                    }
                    _ => {}
                }
            }
        }
        // Writers per resource, in declaration order.
        let mut writers: Vec<Vec<usize>> = vec![Vec::new(); nr];
        for (pi, p) in self.passes.iter().enumerate() {
            for a in p.accesses.iter().filter(|a| a.write) {
                let w = &mut writers[a.res.0 as usize];
                if w.last() != Some(&pi) {
                    w.push(pi);
                }
            }
        }
        // Dependencies.
        let mut deps: Vec<Vec<usize>> = vec![Vec::new(); np];
        for (pi, p) in self.passes.iter().enumerate() {
            for a in &p.accesses {
                let w = &writers[a.res.0 as usize];
                if a.write {
                    if let Some(&prev) = w.iter().take_while(|&&x| x < pi).last() {
                        deps[pi].push(prev);
                    }
                } else {
                    if w.is_empty() && matches!(self.resources[a.res.0 as usize].kind, ResKind::Transient(_)) {
                        return Err(GraphError::ReadBeforeWrite { pass: p.name, res: self.resources[a.res.0 as usize].name });
                    }
                    deps[pi].extend(w.iter().copied().filter(|&x| x != pi));
                }
            }
            deps[pi].sort_unstable();
            deps[pi].dedup();
        }
        // Topological order of all passes (Kahn, lowest declaration index first).
        let mut indeg: Vec<usize> = deps.iter().map(Vec::len).collect();
        let mut users: Vec<Vec<usize>> = vec![Vec::new(); np];
        for (pi, d) in deps.iter().enumerate() {
            for &x in d {
                users[x].push(pi);
            }
        }
        let mut ready: std::collections::BTreeSet<usize> = (0..np).filter(|&i| indeg[i] == 0).collect();
        let mut topo = Vec::with_capacity(np);
        while let Some(i) = ready.pop_first() {
            topo.push(i);
            for &u in &users[i] {
                indeg[u] -= 1;
                if indeg[u] == 0 {
                    ready.insert(u);
                }
            }
        }
        if topo.len() != np {
            let names = (0..np).filter(|i| indeg[*i] > 0).map(|i| self.passes[i].name).collect();
            return Err(GraphError::Cycle(names));
        }
        // Culling: keep roots and everything they depend on.
        let mut needed = vec![false; np];
        let mut stack: Vec<usize> = (0..np)
            .filter(|&i| {
                let p = &self.passes[i];
                p.side_effect
                    || p.accesses.iter().any(|a| a.write && !matches!(self.resources[a.res.0 as usize].kind, ResKind::Transient(_)))
            })
            .collect();
        while let Some(i) = stack.pop() {
            if !needed[i] {
                needed[i] = true;
                stack.extend(deps[i].iter().copied());
            }
        }
        let order: Vec<usize> = topo.iter().copied().filter(|&i| needed[i]).collect();
        let culled: Vec<usize> = (0..np).filter(|&i| !needed[i]).collect();

        // Transient usage and lifetimes over the running passes.
        let mut usage = vec![Usage::default(); nr];
        let mut first = vec![usize::MAX; nr];
        let mut last = vec![0usize; nr];
        for (pos, &pi) in order.iter().enumerate() {
            for a in &self.passes[pi].accesses {
                let r = a.res.0 as usize;
                add_usage(&mut usage[r], a.state);
                first[r] = first[r].min(pos);
                last[r] = last[r].max(pos);
            }
        }
        // Greedy aliasing: by first use, into the first compatible slot that is free.
        let mut transients: Vec<usize> =
            (0..nr).filter(|&r| first[r] != usize::MAX && matches!(self.resources[r].kind, ResKind::Transient(_))).collect();
        transients.sort_by_key(|&r| (first[r], r));
        let mut physical = vec![None; nr];
        let mut slots: Vec<PhysicalSlot> = Vec::new();
        let mut slot_end: Vec<usize> = Vec::new();
        for r in transients {
            let ResKind::Transient(info) = self.resources[r].kind else { unreachable!() };
            let want = PhysicalSlot { info, usage: usage[r] };
            let found = (0..slots.len()).find(|&s| slots[s] == want && slot_end[s] < first[r]);
            let s = match found {
                Some(s) => s,
                None => {
                    slots.push(want);
                    slot_end.push(0);
                    slots.len() - 1
                }
            };
            slot_end[s] = last[r];
            physical[r] = Some(s);
        }

        // Transitions.
        let mut state: Vec<ResourceState> = self
            .resources
            .iter()
            .map(|d| match d.kind {
                ResKind::Transient(_) => ResourceState::Undefined,
                ResKind::ImportTexture { initial, .. } | ResKind::ImportBuffer { initial, .. } => initial,
            })
            .collect();
        let mut before = Vec::with_capacity(order.len());
        for &pi in &order {
            let mut t = Vec::new();
            for a in &self.passes[pi].accesses {
                let r = a.res.0 as usize;
                if state[r] != a.state && !t.iter().any(|x: &PlannedTransition| x.res == a.res) {
                    t.push(PlannedTransition { res: a.res, from: state[r], to: a.state });
                    state[r] = a.state;
                }
            }
            before.push(t);
        }
        let mut after = Vec::new();
        for (r, d) in self.resources.iter().enumerate() {
            let fin = match d.kind {
                ResKind::ImportTexture { final_state, .. } | ResKind::ImportBuffer { final_state, .. } => final_state,
                ResKind::Transient(_) => None,
            };
            if let Some(f) = fin
                && state[r] != f
            {
                after.push(PlannedTransition { res: Res(r as u32), from: state[r], to: f });
                state[r] = f;
            }
        }
        Ok(Plan { order, culled, before, after, physical, slots, final_states: state })
    }

    /// Compiles and records the frame. Returns the plan (for final states of imports).
    pub fn execute(mut self, dev: &dyn Device, enc: &mut dyn CommandEncoder, pool: &mut TransientPool) -> Result<Plan, GraphError> {
        let plan = self.compile()?;
        let phys = pool.acquire(dev, &plan.slots)?;
        let resolved: Vec<Resolved> = self
            .resources
            .iter()
            .enumerate()
            .map(|(r, d)| match d.kind {
                ResKind::Transient(info) => match plan.physical[r] {
                    Some(s) => Resolved::Texture { id: phys[s].0, info, bindless: phys[s].1 },
                    None => Resolved::Unused,
                },
                ResKind::ImportTexture { id, info, .. } => Resolved::Texture { id, info, bindless: None },
                ResKind::ImportBuffer { id, .. } => Resolved::Buffer(id),
            })
            .collect();
        let to_gpu = |t: &PlannedTransition| match resolved[t.res.0 as usize] {
            Resolved::Texture { id, .. } => Transition::Texture { texture: id, sub: None, from: t.from, to: t.to },
            Resolved::Buffer(id) => Transition::Buffer { buffer: id, from: t.from, to: t.to },
            Resolved::Unused => unreachable!("transitions only for used resources"),
        };
        let mut gpu_t: Vec<Transition> = Vec::new();
        enc.begin_timer(FRAME_SCOPE);
        for (pos, &pi) in plan.order.iter().enumerate() {
            gpu_t.clear();
            gpu_t.extend(plan.before[pos].iter().map(to_gpu));
            if !gpu_t.is_empty() {
                enc.transition(&gpu_t);
            }
            let p = &mut self.passes[pi];
            enc.begin_timer(p.name);
            enc.debug_group(p.name);
            let exec = p.exec.take();
            match p.kind {
                PassKind::Raster => {
                    let att = |c: &ColorAtt| match resolved[c.res.0 as usize] {
                        Resolved::Texture { id, .. } => Attachment { texture: id, sub: Subresource { mip: 0, layer: 0 }, load: c.load, store: c.store },
                        _ => unreachable!("validated in compile"),
                    };
                    let colors: Vec<Attachment> = p.colors.iter().map(att).collect();
                    let depth = p.depth.as_ref().map(att);
                    enc.begin_render_pass(&RenderPassDesc { label: p.name, colors: &colors, depth });
                    let size = p.colors.first().or(p.depth.as_ref()).and_then(|c| match resolved[c.res.0 as usize] {
                        Resolved::Texture { info, .. } => Some((info.width, info.height)),
                        _ => None,
                    });
                    if let Some((w, h)) = size {
                        enc.set_viewport(0.0, 0.0, w as f32, h as f32);
                    }
                    if let Some(f) = exec {
                        f(&mut PassCtx { enc: &mut *enc, dev, resolved: &resolved, names: &self.resources });
                    }
                    enc.end_render_pass();
                }
                PassKind::Compute => {
                    enc.begin_compute_pass(p.name);
                    if let Some(f) = exec {
                        f(&mut PassCtx { enc: &mut *enc, dev, resolved: &resolved, names: &self.resources });
                    }
                    enc.end_compute_pass();
                }
                PassKind::Manual => {
                    if let Some(f) = exec {
                        f(&mut PassCtx { enc: &mut *enc, dev, resolved: &resolved, names: &self.resources });
                    }
                }
            }
            enc.end_debug_group();
            enc.end_timer();
        }
        gpu_t.clear();
        gpu_t.extend(plan.after.iter().map(to_gpu));
        if !gpu_t.is_empty() {
            enc.transition(&gpu_t);
        }
        enc.end_timer();
        pool.end_frame(dev);
        Ok(plan)
    }
}

fn add_usage(u: &mut Usage, s: ResourceState) {
    use ResourceState::*;
    match s {
        Undefined | Present => {}
        ShaderRead => u.shader_read = true,
        ShaderWrite => u.shader_write = true,
        RenderTarget => u.render_target = true,
        DepthWrite | DepthRead => u.depth = true,
        IndirectArgs => u.indirect_args = true,
        CopySrc => u.copy_src = true,
        CopyDst => u.copy_dst = true,
    }
}

#[derive(Clone, Copy)]
enum Resolved {
    Texture { id: TextureId, info: TextureInfo, bindless: Option<BindlessTexture> },
    Buffer(BufferId),
    Unused,
}

/// What a pass callback gets: the encoder, the device, and the physical resources.
pub struct PassCtx<'e> {
    pub enc: &'e mut dyn CommandEncoder,
    pub dev: &'e dyn Device,
    resolved: &'e [Resolved],
    names: &'e [ResDecl],
}

impl PassCtx<'_> {
    /// The physical texture of a texture resource.
    pub fn texture(&self, r: Res) -> TextureId {
        match self.resolved[r.0 as usize] {
            Resolved::Texture { id, .. } => id,
            _ => panic!("{} is not a texture used this frame", self.names[r.0 as usize].name),
        }
    }
    pub fn texture_info(&self, r: Res) -> TextureInfo {
        match self.resolved[r.0 as usize] {
            Resolved::Texture { info, .. } => info,
            _ => panic!("{} is not a texture used this frame", self.names[r.0 as usize].name),
        }
    }
    /// Bindless index of a transient that some pass reads or writes in a shader.
    pub fn bindless(&self, r: Res) -> BindlessTexture {
        match self.resolved[r.0 as usize] {
            Resolved::Texture { bindless: Some(b), .. } => b,
            _ => panic!("{} has no bindless index (not a shader-accessed transient)", self.names[r.0 as usize].name),
        }
    }
    pub fn buffer(&self, r: Res) -> BufferId {
        match self.resolved[r.0 as usize] {
            Resolved::Buffer(id) => id,
            _ => panic!("{} is not a buffer", self.names[r.0 as usize].name),
        }
    }
}

struct PoolEntry {
    slot: PhysicalSlot,
    tex: TextureId,
    bindless: Option<BindlessTexture>,
    last_used: u64,
    taken: bool,
}

/// Physical transient textures, kept across frames. A plan's slots are matched to entries
/// with the same shape and usage; entries unused for [`EVICT_AFTER_FRAMES`] frames are
/// destroyed (the device defers the free until no frame in flight uses them).
#[derive(Default)]
pub struct TransientPool {
    entries: Vec<PoolEntry>,
    frame: u64,
    created: u64,
}

impl TransientPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of textures the pool has ever created (warm-up check: stops growing).
    pub fn textures_created(&self) -> u64 {
        self.created
    }
    /// Number of textures alive in the pool.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn acquire(&mut self, dev: &dyn Device, slots: &[PhysicalSlot]) -> Result<Vec<(TextureId, Option<BindlessTexture>)>, GraphError> {
        for e in &mut self.entries {
            e.taken = false;
        }
        let mut out = Vec::with_capacity(slots.len());
        for s in slots {
            let i = match self.entries.iter().position(|e| !e.taken && e.slot == *s) {
                Some(i) => i,
                None => {
                    let tex = dev.create_texture(&TextureDesc {
                        label: "ed_render transient",
                        kind: TextureKind::D2,
                        format: s.info.format,
                        width: s.info.width,
                        height: s.info.height,
                        depth_or_layers: 1,
                        mips: 1,
                        samples: 1,
                        usage: s.usage,
                    })?;
                    let bindless = if s.usage.shader_read || s.usage.shader_write { Some(dev.bind_texture(tex)?) } else { None };
                    self.created += 1;
                    self.entries.push(PoolEntry { slot: *s, tex, bindless, last_used: self.frame, taken: false });
                    self.entries.len() - 1
                }
            };
            let e = &mut self.entries[i];
            e.taken = true;
            e.last_used = self.frame;
            out.push((e.tex, e.bindless));
        }
        Ok(out)
    }

    fn end_frame(&mut self, dev: &dyn Device) {
        let frame = self.frame;
        self.entries.retain(|e| {
            let keep = e.last_used + EVICT_AFTER_FRAMES > frame;
            if !keep {
                Self::free(dev, e);
            }
            keep
        });
        self.frame += 1;
    }

    fn free(dev: &dyn Device, e: &PoolEntry) {
        if let Some(b) = e.bindless {
            dev.unbind_texture(b);
        }
        dev.destroy_texture(e.tex);
    }

    /// Destroy every pooled texture.
    pub fn destroy(&mut self, dev: &dyn Device) {
        for e in self.entries.drain(..) {
            Self::free(dev, &e);
        }
    }
}

/// GPU time of one pass in a finished frame.
#[derive(Clone, Debug, PartialEq)]
pub struct PassTiming {
    pub name: &'static str,
    pub ms: f32,
}

/// Per-pass GPU timings of the most recent finished frame the device has results for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameTimings {
    /// The [`FRAME_SCOPE`] scope: first pass start to last pass end.
    pub frame_ms: f32,
    pub passes: Vec<PassTiming>,
}

impl FrameTimings {
    /// Reads `Device::timings` (results arrive a few frames late) and keeps the frame scope
    /// and the per-pass scopes directly below it.
    pub fn read(dev: &dyn Device, scratch: &mut Vec<Timing>) -> Option<Self> {
        dev.timings(scratch);
        Self::from_timings(scratch)
    }

    pub fn from_timings(t: &[Timing]) -> Option<Self> {
        let frame = t.iter().find(|x| x.depth == 0 && x.label == FRAME_SCOPE)?;
        let passes = t.iter().filter(|x| x.depth == 1).map(|x| PassTiming { name: x.label, ms: x.gpu_ms }).collect();
        Some(Self { frame_ms: frame.gpu_ms, passes })
    }

    pub fn pass_ms(&self, name: &str) -> Option<f32> {
        self.passes.iter().find(|p| p.name == name).map(|p| p.ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ResourceState::*;
    use std::num::NonZeroU32;

    const HDR: TextureInfo = TextureInfo { format: Format::Rgba16Float, width: 64, height: 64 };
    const DEPTH: TextureInfo = TextureInfo { format: Format::Depth32Float, width: 64, height: 64 };
    const OUT: TextureInfo = TextureInfo { format: Format::Rgba8Srgb, width: 64, height: 64 };

    fn out_tex() -> TextureId {
        TextureId::from_parts(0, NonZeroU32::new(1).unwrap())
    }

    fn names(g: &FrameGraph<'_>, idx: &[usize]) -> Vec<&'static str> {
        idx.iter().map(|&i| g.passes[i].name).collect()
    }

    /// The renderer's shape, declared deliberately out of order.
    fn renderer_like(g: &mut FrameGraph<'_>) -> (Res, Res, Res) {
        let hdr = g.create_texture("hdr", HDR);
        let depth = g.create_texture("depth", DEPTH);
        let out = g.import_texture("out", out_tex(), OUT, Undefined, Some(Present));
        g.add_pass("tonemap", PassKind::Raster).read(hdr, ShaderRead).color(out, LoadOp::Discard, StoreOp::Store).done();
        g.add_pass("sky", PassKind::Raster).color(hdr, LoadOp::Clear([0.0; 4]), StoreOp::Store).done();
        g.add_pass("scene", PassKind::Raster).color(hdr, LoadOp::Load, StoreOp::Store).depth(depth, LoadOp::Clear([0.0; 4]), StoreOp::Discard, true).done();
        g.add_pass("overlay", PassKind::Manual).write(out, RenderTarget).done();
        (hdr, depth, out)
    }

    #[test]
    fn orders_from_dependencies_not_declaration() {
        let mut g = FrameGraph::new();
        renderer_like(&mut g);
        let plan = g.compile().unwrap();
        assert_eq!(names(&g, &plan.order), ["sky", "scene", "tonemap", "overlay"]);
        assert!(plan.culled.is_empty());
    }

    #[test]
    fn cycle_is_an_error() {
        let mut g = FrameGraph::new();
        let a = g.create_texture("a", HDR);
        let b = g.create_texture("b", HDR);
        let out = g.import_texture("out", out_tex(), OUT, Undefined, None);
        g.add_pass("p1", PassKind::Raster).read(a, ShaderRead).color(b, LoadOp::Discard, StoreOp::Store).done();
        g.add_pass("p2", PassKind::Raster).read(b, ShaderRead).color(a, LoadOp::Discard, StoreOp::Store).done();
        g.add_pass("final", PassKind::Raster).read(a, ShaderRead).color(out, LoadOp::Discard, StoreOp::Store).done();
        match g.compile() {
            Err(GraphError::Cycle(p)) => {
                assert!(p.contains(&"p1") && p.contains(&"p2"), "{p:?}");
            }
            other => panic!("expected a cycle, got {other:?}"),
        }
    }

    #[test]
    fn conflicting_states_in_one_pass_are_an_error() {
        let mut g = FrameGraph::new();
        let a = g.create_texture("a", HDR);
        g.add_pass("p", PassKind::Manual).write(a, ShaderWrite).read(a, ShaderRead).side_effect().done();
        assert_eq!(g.compile().unwrap_err(), GraphError::ConflictingAccess { pass: "p", res: "a" });
    }

    #[test]
    fn reading_an_unwritten_transient_is_an_error() {
        let mut g = FrameGraph::new();
        let a = g.create_texture("a", HDR);
        g.add_pass("p", PassKind::Manual).read(a, ShaderRead).side_effect().done();
        assert!(matches!(g.compile(), Err(GraphError::ReadBeforeWrite { .. })));
    }

    #[test]
    fn aliases_non_overlapping_transients() {
        // a -> b -> c -> out: `a` dies when `b` is made, so `c` (same shape) can reuse `a`.
        let mut g = FrameGraph::new();
        let a = g.create_texture("a", HDR);
        let b = g.create_texture("b", HDR);
        let c = g.create_texture("c", HDR);
        let out = g.import_texture("out", out_tex(), OUT, Undefined, None);
        g.add_pass("make a", PassKind::Raster).color(a, LoadOp::Discard, StoreOp::Store).done();
        g.add_pass("a to b", PassKind::Raster).read(a, ShaderRead).color(b, LoadOp::Discard, StoreOp::Store).done();
        g.add_pass("b to c", PassKind::Raster).read(b, ShaderRead).color(c, LoadOp::Discard, StoreOp::Store).done();
        g.add_pass("c to out", PassKind::Raster).read(c, ShaderRead).color(out, LoadOp::Discard, StoreOp::Store).done();
        let plan = g.compile().unwrap();
        assert_eq!(plan.slots.len(), 2, "three transients, two physical textures");
        assert_eq!(plan.physical[a.0 as usize], plan.physical[c.0 as usize]);
        assert_ne!(plan.physical[a.0 as usize], plan.physical[b.0 as usize]);
        assert_ne!(plan.physical[b.0 as usize], plan.physical[c.0 as usize]);
    }

    #[test]
    fn does_not_alias_overlapping_or_different_shapes() {
        let mut g = FrameGraph::new();
        let (hdr, depth, _) = renderer_like(&mut g);
        let plan = g.compile().unwrap();
        assert_eq!(plan.slots.len(), 2);
        assert_ne!(plan.physical[hdr.0 as usize], plan.physical[depth.0 as usize]);
        let s = plan.slots[plan.physical[hdr.0 as usize].unwrap()];
        assert!(s.usage.render_target && s.usage.shader_read && !s.usage.depth);
    }

    #[test]
    fn transitions_exactly_where_state_changes() {
        let mut g = FrameGraph::new();
        let (hdr, depth, out) = renderer_like(&mut g);
        let plan = g.compile().unwrap();
        let t = |res, from, to| PlannedTransition { res, from, to };
        // sky, scene, tonemap, overlay
        assert_eq!(plan.before[0], vec![t(hdr, Undefined, RenderTarget)]);
        assert_eq!(plan.before[1], vec![t(depth, Undefined, DepthWrite)], "hdr stays RenderTarget: no transition");
        assert_eq!(plan.before[2], vec![t(hdr, RenderTarget, ShaderRead), t(out, Undefined, RenderTarget)]);
        assert!(plan.before[3].is_empty(), "overlay keeps out in RenderTarget");
        assert_eq!(plan.after, vec![t(out, RenderTarget, Present)]);
        assert_eq!(plan.transition_count(), 5);
        assert_eq!(plan.final_states[out.0 as usize], Present);
    }

    #[test]
    fn imported_state_already_right_needs_no_transition() {
        let mut g = FrameGraph::new();
        let buf = g.import_buffer("instances", BufferId::from_parts(0, NonZeroU32::new(1).unwrap()), ShaderRead, None);
        let out = g.import_texture("out", out_tex(), OUT, RenderTarget, None);
        g.add_pass("draw", PassKind::Raster).read(buf, ShaderRead).color(out, LoadOp::Load, StoreOp::Store).done();
        let plan = g.compile().unwrap();
        assert_eq!(plan.transition_count(), 0);
    }

    #[test]
    fn culls_passes_whose_outputs_are_unused() {
        let mut g = FrameGraph::new();
        let (hdr, _, _) = renderer_like(&mut g);
        let debug = g.create_texture("debug view", HDR);
        g.add_pass("debug", PassKind::Raster).read(hdr, ShaderRead).color(debug, LoadOp::Discard, StoreOp::Store).done();
        let unused = g.create_texture("unused", HDR);
        g.add_pass("orphan", PassKind::Compute).write(unused, ShaderWrite).done();
        g.add_pass("readback", PassKind::Manual).read(hdr, CopySrc).side_effect().done();
        let plan = g.compile().unwrap();
        assert_eq!(names(&g, &plan.culled), ["debug", "orphan"]);
        assert_eq!(names(&g, &plan.order), ["sky", "scene", "tonemap", "overlay", "readback"]);
        assert_eq!(plan.physical[debug.0 as usize], None);
        assert_eq!(plan.physical[unused.0 as usize], None);
    }

    #[test]
    fn writers_keep_declaration_order() {
        let mut g = FrameGraph::new();
        let out = g.import_texture("out", out_tex(), OUT, Undefined, None);
        g.add_pass("first", PassKind::Raster).color(out, LoadOp::Clear([0.0; 4]), StoreOp::Store).done();
        g.add_pass("second", PassKind::Raster).color(out, LoadOp::Load, StoreOp::Store).done();
        g.add_pass("third", PassKind::Raster).color(out, LoadOp::Load, StoreOp::Store).done();
        let plan = g.compile().unwrap();
        assert_eq!(names(&g, &plan.order), ["first", "second", "third"]);
    }

    #[test]
    fn frame_timings_pick_depth_one_scopes() {
        let t = [
            Timing { label: "sky", depth: 1, gpu_ms: 0.1 },
            Timing { label: "scene", depth: 1, gpu_ms: 0.5 },
            Timing { label: FRAME_SCOPE, depth: 0, gpu_ms: 0.7 },
        ];
        let f = FrameTimings::from_timings(&t).unwrap();
        assert_eq!(f.frame_ms, 0.7);
        assert_eq!(f.pass_ms("scene"), Some(0.5));
        assert_eq!(f.passes.len(), 2);
        assert!(FrameTimings::from_timings(&t[..2]).is_none());
    }
}
