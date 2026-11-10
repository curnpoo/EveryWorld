//! Pass graph (m2.md rule 3). Passes are declared in submission order with the resources they
//! read and write. `compile` validates the declaration, culls passes that feed no output,
//! computes transient lifetimes and an alias plan (which transients can share memory).
//!
//! Backends execute `CompiledGraph::order`. wgpu allocates one texture per alias slot; Metal
//! maps a slot to a heap-aliased allocation and can move independent compute passes to a
//! second queue (`QueueKind::Compute`).

use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResourceId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PassId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    R8,
    Rg8,
    Rgba8,
    R16F,
    Rg16F,
    Rgba16F,
    R32F,
    R32Uint,
    Rg32Uint,
    Depth32F,
}

impl Format {
    pub fn bytes_per_pixel(self) -> u32 {
        match self {
            Format::R8 => 1,
            Format::Rg8 | Format::R16F => 2,
            Format::Rgba8 | Format::Rg16F | Format::R32F | Format::R32Uint | Format::Depth32F => 4,
            Format::Rgba16F | Format::Rg32Uint => 8,
        }
    }
}

/// Size relative to the frame, so dynamic resolution (S6) resizes the graph without edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Size {
    /// Internal render resolution (after the dynamic-resolution scale).
    Render,
    /// Display resolution (post-upscale).
    Display,
    /// Render resolution divided by `n` (half-res clouds, SSAO).
    RenderDiv(u32),
    Fixed(u32, u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifetime {
    /// Lives across frames or outside the graph (swapchain, history, pools). Never aliased.
    Imported,
    /// Exists only inside this frame's graph; eligible for aliasing.
    Transient,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueKind {
    Graphics,
    /// Eligible for an async compute queue when the backend has one.
    Compute,
}

#[derive(Clone, Debug)]
pub struct ResourceDesc {
    pub name: String,
    pub format: Format,
    pub size: Size,
    pub lifetime: Lifetime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dims {
    pub render: (u32, u32),
    pub display: (u32, u32),
}

impl Size {
    pub fn resolve(self, d: Dims) -> (u32, u32) {
        match self {
            Size::Render => d.render,
            Size::Display => d.display,
            Size::RenderDiv(n) => ((d.render.0 / n.max(1)).max(1), (d.render.1 / n.max(1)).max(1)),
            Size::Fixed(w, h) => (w, h),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// A pass names a resource that does not exist in this builder.
    UnknownResource { pass: String },
    /// A transient resource is read before any earlier pass writes it.
    ReadBeforeWrite { pass: String, resource: String },
    /// Two passes share a name (timers and logs are keyed by name).
    DuplicatePass { name: String },
    /// A pass writes and reads nothing.
    EmptyPass { name: String },
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::UnknownResource { pass } => write!(f, "pass `{pass}` uses an unknown resource"),
            GraphError::ReadBeforeWrite { pass, resource } => {
                write!(f, "pass `{pass}` reads transient `{resource}` before any pass writes it")
            }
            GraphError::DuplicatePass { name } => write!(f, "duplicate pass name `{name}`"),
            GraphError::EmptyPass { name } => write!(f, "pass `{name}` declares no reads or writes"),
        }
    }
}
impl std::error::Error for GraphError {}

#[derive(Clone, Debug)]
struct PassDecl {
    name: String,
    queue: QueueKind,
    reads: Vec<ResourceId>,
    writes: Vec<ResourceId>,
    /// Always runs (has an effect outside the graph), e.g. a debug readback.
    force: bool,
}

#[derive(Default, Debug)]
pub struct GraphBuilder {
    resources: Vec<ResourceDesc>,
    passes: Vec<PassDecl>,
}

/// Declares one pass; call `.done()` (or drop through `GraphBuilder::pass`'s closure form).
pub struct PassBuilder<'a> {
    g: &'a mut GraphBuilder,
    decl: PassDecl,
}

impl<'a> PassBuilder<'a> {
    pub fn reads(mut self, r: ResourceId) -> Self {
        self.decl.reads.push(r);
        self
    }
    pub fn writes(mut self, r: ResourceId) -> Self {
        self.decl.writes.push(r);
        self
    }
    pub fn queue(mut self, q: QueueKind) -> Self {
        self.decl.queue = q;
        self
    }
    /// Keep this pass even if nothing reads its outputs.
    pub fn force(mut self) -> Self {
        self.decl.force = true;
        self
    }
    pub fn done(self) -> PassId {
        let id = PassId(self.g.passes.len() as u32);
        self.g.passes.push(self.decl);
        id
    }
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    fn add(&mut self, name: &str, format: Format, size: Size, lifetime: Lifetime) -> ResourceId {
        let id = ResourceId(self.resources.len() as u32);
        self.resources.push(ResourceDesc { name: name.to_owned(), format, size, lifetime });
        id
    }

    /// A resource owned outside the graph. Writing it makes a pass a root (it has an effect).
    pub fn import(&mut self, name: &str, format: Format, size: Size) -> ResourceId {
        self.add(name, format, size, Lifetime::Imported)
    }

    pub fn transient(&mut self, name: &str, format: Format, size: Size) -> ResourceId {
        self.add(name, format, size, Lifetime::Transient)
    }

    pub fn pass(&mut self, name: &str) -> PassBuilder<'_> {
        PassBuilder {
            decl: PassDecl { name: name.to_owned(), queue: QueueKind::Graphics, reads: vec![], writes: vec![], force: false },
            g: self,
        }
    }

    /// Validate, cull, and plan memory. `dims` fixes sizes for the alias plan.
    pub fn compile(&self, dims: Dims) -> Result<CompiledGraph, GraphError> {
        let n_res = self.resources.len() as u32;
        let mut seen = HashSet::new();
        for p in &self.passes {
            if !seen.insert(p.name.as_str()) {
                return Err(GraphError::DuplicatePass { name: p.name.clone() });
            }
            if p.reads.is_empty() && p.writes.is_empty() {
                return Err(GraphError::EmptyPass { name: p.name.clone() });
            }
            if p.reads.iter().chain(&p.writes).any(|r| r.0 >= n_res) {
                return Err(GraphError::UnknownResource { pass: p.name.clone() });
            }
        }
        // Read-before-write on transients.
        let mut written = vec![false; n_res as usize];
        for p in &self.passes {
            for r in &p.reads {
                let d = &self.resources[r.0 as usize];
                if d.lifetime == Lifetime::Transient && !written[r.0 as usize] && !p.writes.contains(r) {
                    return Err(GraphError::ReadBeforeWrite { pass: p.name.clone(), resource: d.name.clone() });
                }
            }
            for w in &p.writes {
                written[w.0 as usize] = true;
            }
        }
        // Cull backwards from roots (passes writing imported resources, or forced).
        let mut keep = vec![false; self.passes.len()];
        let mut needed: HashSet<ResourceId> = HashSet::new();
        for (i, p) in self.passes.iter().enumerate().rev() {
            let root = p.force || p.writes.iter().any(|w| self.resources[w.0 as usize].lifetime == Lifetime::Imported);
            if root || p.writes.iter().any(|w| needed.contains(w)) {
                keep[i] = true;
                for w in &p.writes {
                    needed.remove(w);
                }
                needed.extend(p.reads.iter().copied());
            }
        }
        let order: Vec<PassId> = (0..self.passes.len()).filter(|&i| keep[i]).map(|i| PassId(i as u32)).collect();
        let culled: Vec<String> = (0..self.passes.len()).filter(|&i| !keep[i]).map(|i| self.passes[i].name.clone()).collect();

        // Transient lifetimes over positions in `order`.
        let mut first = vec![usize::MAX; n_res as usize];
        let mut last = vec![0usize; n_res as usize];
        for (pos, pid) in order.iter().enumerate() {
            let p = &self.passes[pid.0 as usize];
            for r in p.reads.iter().chain(&p.writes) {
                let i = r.0 as usize;
                first[i] = first[i].min(pos);
                last[i] = last[i].max(pos);
            }
        }
        // Greedy alias assignment: sort transients by first use, reuse a slot whose last use is
        // strictly earlier and whose (format, resolved size) matches exactly.
        let mut live: Vec<usize> = (0..n_res as usize)
            .filter(|&i| self.resources[i].lifetime == Lifetime::Transient && first[i] != usize::MAX)
            .collect();
        live.sort_by_key(|&i| (first[i], i));
        let mut slot_of: Vec<Option<usize>> = vec![None; n_res as usize];
        let mut slots: Vec<Slot> = Vec::new();
        for i in live {
            let d = &self.resources[i];
            let size = d.size.resolve(dims);
            let found = slots.iter().position(|s| s.format == d.format && s.size == size && s.free_after < first[i]);
            let si = match found {
                Some(si) => si,
                None => {
                    slots.push(Slot { format: d.format, size, free_after: 0 });
                    slots.len() - 1
                }
            };
            slots[si].free_after = last[i];
            slot_of[i] = Some(si);
        }
        let bytes = |format: Format, size: (u32, u32)| format.bytes_per_pixel() as u64 * size.0 as u64 * size.1 as u64;
        let bytes_unaliased = (0..n_res as usize)
            .filter(|&i| slot_of[i].is_some())
            .map(|i| bytes(self.resources[i].format, self.resources[i].size.resolve(dims)))
            .sum();
        let bytes_aliased = slots.iter().map(|s| bytes(s.format, s.size)).sum();

        Ok(CompiledGraph {
            order,
            culled,
            slot_of,
            slot_count: slots.len(),
            bytes_unaliased,
            bytes_aliased,
            passes: self.passes.iter().map(|p| PassInfo { name: p.name.clone(), queue: p.queue, reads: p.reads.clone(), writes: p.writes.clone() }).collect(),
            resources: self.resources.clone(),
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    format: Format,
    size: (u32, u32),
    free_after: usize,
}

#[derive(Clone, Debug)]
pub struct PassInfo {
    pub name: String,
    pub queue: QueueKind,
    pub reads: Vec<ResourceId>,
    pub writes: Vec<ResourceId>,
}

/// What a backend executes.
#[derive(Clone, Debug)]
pub struct CompiledGraph {
    /// Kept passes in submission order.
    pub order: Vec<PassId>,
    /// Names of culled passes (logged once; a culled pass is usually a wiring bug).
    pub culled: Vec<String>,
    /// Alias slot per resource (None for imported or unused).
    pub slot_of: Vec<Option<usize>>,
    pub slot_count: usize,
    pub bytes_unaliased: u64,
    pub bytes_aliased: u64,
    passes: Vec<PassInfo>,
    resources: Vec<ResourceDesc>,
}

impl CompiledGraph {
    pub fn pass(&self, id: PassId) -> &PassInfo {
        &self.passes[id.0 as usize]
    }
    pub fn resource(&self, id: ResourceId) -> &ResourceDesc {
        &self.resources[id.0 as usize]
    }
    /// Names in execution order (timers and HUD use this).
    pub fn pass_names(&self) -> Vec<&str> {
        self.order.iter().map(|&p| self.pass(p).name.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: Dims = Dims { render: (1600, 900), display: (2400, 1350) };

    fn frame() -> (GraphBuilder, ResourceId) {
        let mut g = GraphBuilder::new();
        let out = g.import("swapchain", Format::Rgba8, Size::Display);
        (g, out)
    }

    #[test]
    fn orders_and_culls_unused_passes() {
        let (mut g, out) = frame();
        let depth = g.transient("depth", Format::Depth32F, Size::Render);
        let color = g.transient("color", Format::Rgba16F, Size::Render);
        let dead = g.transient("dead", Format::Rgba8, Size::Render);
        g.pass("prepass").writes(depth).done();
        g.pass("opaque").reads(depth).writes(color).done();
        g.pass("unused_debug").reads(depth).writes(dead).done();
        g.pass("tonemap").reads(color).writes(out).done();
        let c = g.compile(D).unwrap();
        assert_eq!(c.pass_names(), ["prepass", "opaque", "tonemap"]);
        assert_eq!(c.culled, ["unused_debug"]);
    }

    #[test]
    fn force_keeps_a_pass_without_readers() {
        let (mut g, _out) = frame();
        let t = g.transient("t", Format::R32F, Size::Render);
        g.pass("readback").writes(t).force().done();
        assert_eq!(g.compile(D).unwrap().pass_names(), ["readback"]);
    }

    #[test]
    fn read_before_write_is_an_error() {
        let (mut g, out) = frame();
        let t = g.transient("t", Format::Rgba8, Size::Render);
        g.pass("early").reads(t).writes(out).done();
        g.pass("late").writes(t).done();
        assert_eq!(g.compile(D).unwrap_err(), GraphError::ReadBeforeWrite { pass: "early".into(), resource: "t".into() });
    }

    #[test]
    fn duplicate_empty_and_unknown_are_errors() {
        let (mut g, out) = frame();
        g.pass("a").writes(out).done();
        g.pass("a").writes(out).done();
        assert!(matches!(g.compile(D), Err(GraphError::DuplicatePass { .. })));
        let (mut g, _) = frame();
        g.pass("e").done();
        assert!(matches!(g.compile(D), Err(GraphError::EmptyPass { .. })));
        let (mut g, _) = frame();
        g.pass("u").writes(ResourceId(99)).done();
        assert!(matches!(g.compile(D), Err(GraphError::UnknownResource { .. })));
    }

    #[test]
    fn transients_with_disjoint_lifetimes_share_a_slot() {
        let (mut g, out) = frame();
        // a is dead after p2; b (same format/size) starts at p3 -> shares a's slot.
        let a = g.transient("a", Format::Rgba16F, Size::Render);
        let b = g.transient("b", Format::Rgba16F, Size::Render);
        let c = g.transient("c", Format::Rgba16F, Size::Render);
        g.pass("p1").writes(a).done();
        g.pass("p2").reads(a).writes(b).done();
        g.pass("p3").reads(b).writes(c).done();
        g.pass("p4").reads(c).writes(out).done();
        let g = g.compile(D).unwrap();
        // a:[0,1] b:[1,2] c:[2,3]: a and c may share (a ends at 1 < c starts at 2); b overlaps both.
        assert_eq!(g.slot_count, 2);
        // Resource 0 is the swapchain, so a, b, c are 1, 2, 3.
        assert!(g.slot_of[1].is_some());
        assert_eq!(g.slot_of[1], g.slot_of[3]);
        assert_ne!(g.slot_of[1], g.slot_of[2]);
        assert!(g.bytes_aliased < g.bytes_unaliased);
        assert_eq!(g.bytes_unaliased, 3 * 8 * 1600 * 900);
    }

    #[test]
    fn different_format_or_size_never_aliases() {
        let (mut g, out) = frame();
        let a = g.transient("a", Format::Rgba16F, Size::Render);
        let b = g.transient("b", Format::Rgba16F, Size::RenderDiv(2));
        let c = g.transient("c", Format::Rgba8, Size::Render);
        g.pass("p1").writes(a).done();
        g.pass("p2").reads(a).writes(b).done();
        g.pass("p3").reads(b).writes(c).done();
        g.pass("p4").reads(c).writes(out).done();
        assert_eq!(g.compile(D).unwrap().slot_count, 3);
    }

    #[test]
    fn dynamic_resolution_changes_bytes_not_the_graph() {
        let (mut g, out) = frame();
        let t = g.transient("t", Format::Rgba16F, Size::Render);
        g.pass("a").writes(t).done();
        g.pass("b").reads(t).writes(out).done();
        let full = g.compile(D).unwrap();
        let low = g.compile(Dims { render: (800, 450), display: D.display }).unwrap();
        assert_eq!(full.pass_names(), low.pass_names());
        assert_eq!(full.bytes_aliased, 4 * low.bytes_aliased);
    }

    #[test]
    fn read_modify_write_keeps_earlier_writer() {
        let (mut g, out) = frame();
        let t = g.transient("t", Format::Rgba8, Size::Render);
        g.pass("init").writes(t).done();
        g.pass("blend").reads(t).writes(t).done();
        g.pass("present").reads(t).writes(out).done();
        assert_eq!(g.compile(D).unwrap().pass_names(), ["init", "blend", "present"]);
    }
}
