//! Tree impostors (trees G5, docs/plans/trees.md §3 T2): one compact record per tree, its own
//! GPU cull (`imp_cull`, cull.slang) and one indirect quad draw per view and cascade.
//!
//! Every resident tree lives here, whether or not a bank mesh instance also draws it: the cull
//! picks the impostor by the tree's projected height (T1 -> T2 band) and lets it leave by its
//! hash into the far rung (T2 -> T3 band). The bands, the far rung's lift and canopy paint
//! ramps come from one table ([`ImpostorBands`]) the caller sets once, written into
//! `FrameData.imp` every frame (common.slang `ImpData`).

use crate::Tracked;
use crate::alloc::RangeAlloc;
use crate::gpu::{self, IMP_WORDS};
use crate::scene::NodeSlot;
use ed_gpu::{Device, ResourceState, Usage};
use glam::Vec3;

/// Most impostor trees (slots in the tree buffer and in each visible list).
pub const MAX_IMPOSTOR_TREES: u32 = 1 << 22;
/// Most impostor kinds (12 bits of `GpuTree::node_kind`).
pub const MAX_IMPOSTOR_KINDS: u32 = 4096;
const TREE_WORDS: usize = 9;
const KIND_WORDS: usize = 16;
/// Draw lists: the view, then the four cascades.
pub(crate) const IMP_LISTS: u32 = 5;

/// One impostor kind: a baked octahedral atlas (three BC5 textures, bindless indices).
#[derive(Clone, Copy, Debug)]
pub struct ImpostorKind {
    /// Bindless texture indices: A (coverage, depth), B (normal), C (value, bark).
    pub tex: [u32; 3],
    pub frames: u32,
    pub frame_px: u32,
    pub mips: u32,
    /// Sphere centre height and radius (tree space), tree height (m at scale 1).
    pub center_y: f32,
    pub radius: f32,
    pub height: f32,
    /// Linear leaf and bark albedo (the same table as the bank meshes' materials).
    pub leaf: [f32; 3],
    pub bark: [f32; 3],
    /// Crown base height, crown radius and the widest leaf's distance from the trunk axis (m at
    /// scale 1): the far-cascade shadow proxy, and the quads' width (no wider than the crown).
    pub crown_base: f32,
    pub crown_radius: f32,
    pub crown_max: f32,
}

/// One tree for the impostor buffer.
#[derive(Clone, Copy, Debug)]
pub struct ImpostorTree {
    /// Base position relative to its node origin (render frame).
    pub pos: Vec3,
    pub kind: u32,
    /// Per-tree hash (its T2 -> T3 exit threshold).
    pub hash: u8,
    pub scale: f32,
    pub up: Vec3,
    /// Yaw about `up` (radians) and crown widen (x, z scale over y; 0.5..1.5).
    pub yaw: f32,
    pub widen: f32,
    /// The mesh instance's user word (value, hue, drop, phase).
    pub user: u32,
    /// The season word (`ed_world::season::season_word`).
    pub season: u32,
}

/// A block of tree slots, freed as one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImpostorRange {
    pub start: u32,
    pub len: u32,
}

/// The one table of tree hand-off thresholds (pixels of projected height): it is the only
/// source of these numbers for the cull, the resolve and the far rung.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImpostorBands {
    /// T1 -> T2: impostor whole under `t2_in.0` px of the tree's own height, none over `t2_in.1`.
    pub t2_in: (f32, f32),
    /// T2 -> T3: each tree leaves at a hashed reference-tree size in `t3_out.0..t3_out.1` px.
    pub t3_out: (f32, f32),
    /// Far rung crown lift: none at `lift.0`, full at `lift.1` reference px.
    pub lift: (f32, f32),
    /// Far rung canopy paint: none at `paint.0`, full at `paint.1` reference px.
    pub paint: (f32, f32),
    /// The reference tree height (m) the T3 bands are measured with.
    pub ref_tree_m: f32,
    /// Frame pad (the bake's `IMP_PAD`) and value range (`VALUE_MAX`).
    pub pad: f32,
    pub value_max: f32,
    /// Impostors cast sun shadows.
    pub shadows: bool,
    /// From this cascade on, impostors cast every tree's shadow (meshes only nearer).
    pub shadow_cascade: u32,
    /// View impostors move this many pixels' worth of distance (`px * dist / ppr` m) toward
    /// the viewer along their own view ray: the terrain draws coarser than the ground the trees
    /// stand on and would bury them.
    pub ground_pull_px: f32,
}

pub(crate) struct ImpostorStore {
    pub trees: Option<Tracked>,
    pub kinds: Option<Tracked>,
    pub quad: Option<Tracked>,
    alloc: RangeAlloc,
    kind_count: u32,
    pub bands: Option<ImpostorBands>,
    pub on: bool,
}

impl ImpostorStore {
    pub fn new() -> Self {
        Self { trees: None, kinds: None, quad: None, alloc: RangeAlloc::new(MAX_IMPOSTOR_TREES), kind_count: 0, bands: None, on: true }
    }

    fn ensure(&mut self, dev: &dyn Device) -> Result<(), ed_gpu::Error> {
        if self.trees.is_some() {
            return Ok(());
        }
        let u = Usage { shader_read: true, copy_dst: true, ..Default::default() };
        let mut trees = Tracked::new(dev, "impostor trees", MAX_IMPOSTOR_TREES as u64 * (TREE_WORDS * 4) as u64, u)?;
        // Every slot dead (scale 0) until a range is written.
        dev.write_buffer(trees.id, 0, &vec![0u8; MAX_IMPOSTOR_TREES as usize * TREE_WORDS * 4])?;
        trees.state = ResourceState::CopyDst;
        let kinds = Tracked::new(dev, "impostor kinds", MAX_IMPOSTOR_KINDS as u64 * (KIND_WORDS * 4) as u64, u)?;
        let mut quad = Tracked::new(dev, "impostor quad", 24, u)?;
        dev.write_buffer(quad.id, 0, &gpu::bytes(&[0u32, 1, 2, 2, 1, 3]))?;
        quad.state = ResourceState::CopyDst;
        self.trees = Some(trees);
        self.kinds = Some(kinds);
        self.quad = Some(quad);
        Ok(())
    }

    pub fn add_kind(&mut self, dev: &dyn Device, k: &ImpostorKind) -> Result<u32, ed_gpu::Error> {
        self.ensure(dev)?;
        if self.kind_count >= MAX_IMPOSTOR_KINDS {
            return Err(ed_gpu::Error::Invalid("impostor kind table full".into()));
        }
        let f = |x: f32| x.to_bits();
        // Packed 10:11:11 bits, base over the height, radii over 1.5 x the height.
        let h = k.height.max(1e-3);
        let un = |x: f32, m: f32| (x.clamp(0.0, 1.0) * m).round() as u32;
        let crown = un(k.crown_base / h, 1023.0) | (un(k.crown_radius / (1.5 * h), 2047.0) << 10) | (un(k.crown_max / (1.5 * h), 2047.0) << 21);
        let w: [u32; KIND_WORDS] = [
            k.tex[0], k.tex[1], k.tex[2], k.frames,
            f(k.center_y), f(k.radius), f(k.height), f(k.frame_px as f32),
            f(k.leaf[0]), f(k.leaf[1]), f(k.leaf[2]), f(k.mips as f32),
            f(k.bark[0]), f(k.bark[1]), f(k.bark[2]), crown,
        ];
        let id = self.kind_count;
        let t = self.kinds.as_ref().expect("ensured");
        dev.write_buffer(t.id, id as u64 * (KIND_WORDS * 4) as u64, &gpu::bytes(&w))?;
        self.kind_count += 1;
        Ok(id)
    }

    pub fn add_trees(&mut self, dev: &dyn Device, node: NodeSlot, trees: &[ImpostorTree]) -> Result<Option<ImpostorRange>, ed_gpu::Error> {
        self.ensure(dev)?;
        let n = trees.len() as u32;
        let Some(start) = self.alloc.alloc(n) else { return Ok(None) };
        let mut w = Vec::with_capacity(trees.len() * TREE_WORDS);
        for t in trees {
            let yaw = (t.yaw.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * 65535.0).round() as u32 & 0xFFFF;
            let wd = ((t.widen - 0.5).clamp(0.0, 1.0) * 65535.0).round() as u32;
            w.extend_from_slice(&[
                t.pos.x.to_bits(),
                t.pos.y.to_bits(),
                t.pos.z.to_bits(),
                (node.0 as u32 & 0xFFF) | ((t.kind & 0xFFF) << 12) | ((t.hash as u32) << 24),
                t.scale.to_bits(),
                gpu::oct_encode(t.up),
                yaw | (wd << 16),
                t.user,
                t.season,
            ]);
        }
        let tb = self.trees.as_ref().expect("ensured");
        dev.write_buffer(tb.id, start as u64 * (TREE_WORDS * 4) as u64, &gpu::bytes(&w))?;
        Ok(Some(ImpostorRange { start, len: n }))
    }

    /// Kills the range's trees at once (scale 0); the slots are reused after frames in flight.
    pub fn kill(&mut self, dev: &dyn Device, r: ImpostorRange) -> Result<(), ed_gpu::Error> {
        if let Some(tb) = &self.trees {
            dev.write_buffer(tb.id, r.start as u64 * (TREE_WORDS * 4) as u64, &vec![0u8; r.len as usize * TREE_WORDS * 4])?;
        }
        Ok(())
    }
    pub fn free(&mut self, r: ImpostorRange) {
        self.alloc.free(r.start, r.len);
    }

    /// Slots the cull walks (high-water mark), 0 when nothing to draw.
    pub fn slots(&self) -> u32 {
        if self.on && self.bands.is_some() && self.trees.is_some() { self.alloc.high_water() } else { 0 }
    }
    pub fn used(&self) -> u32 {
        self.alloc.used()
    }

    /// `FrameData.imp` words.
    pub fn words(&self) -> [u32; IMP_WORDS] {
        let mut w = [0u32; IMP_WORDS];
        let Some(b) = self.bands else { return w };
        if let (Some(t), Some(k)) = (&self.trees, &self.kinds) {
            w[0] = t.bindless.0;
            w[1] = k.bindless.0;
        }
        w[2] = self.slots();
        // Flags in bits 0..8, the ground pull (1/16 px) above them.
        let pull = ((b.ground_pull_px.max(0.0) * 16.0).round() as u32).min(0xFF_FFFF) << 8;
        w[3] = if w[2] > 0 { 1 | (if b.shadows { 2 } else { 0 }) | pull } else { 0 };
        let f = [b.t2_in.0, b.t2_in.1, b.t3_out.0, b.t3_out.1, b.lift.0, b.lift.1, b.paint.0, b.paint.1, b.ref_tree_m, b.pad, b.value_max, b.shadow_cascade as f32];
        for (i, x) in f.iter().enumerate() {
            w[4 + i] = x.to_bits();
        }
        w
    }

    pub fn destroy(self, dev: &dyn Device) {
        for t in [self.trees, self.kinds, self.quad].into_iter().flatten() {
            t.destroy(dev);
        }
    }
}
