//! Grass style A (procedural blades, `ed_world::grass`): one 32-byte record per clump, its own
//! GPU cull (`grass_cull`, cull.slang) into blade-count buckets, and one indexed draw per bucket
//! with no vertex buffer: the vertex shader (visibility.slang `vs_grass`) turns the vertex id
//! into (blade, segment, side) and builds the blade from the clump's seed and its type row.
//! The resolve (`grass_pixel`) rebuilds the same blade from the visibility word.
//!
//! Buckets (mirrored by common.slang `GRASS_SLOTS`, `GRASS_SEGS`): a clump draws in the
//! smallest bucket with at least its continuous blade count; blades past the count shrink
//! away. Lists: the view's buckets, then the nearest cascade's (shadows: one bucket coarser).

use crate::Tracked;
use crate::alloc::RangeAlloc;
use crate::gpu::{self, GRASS_WORDS};
use crate::scene::NodeSlot;
use ed_gpu::{Device, ResourceState, Usage};
use glam::Vec3;

/// Most clump slots.
pub const MAX_GRASS_CLUMPS: u32 = 1 << 20;
/// Most grass types (8 bits of the record).
pub const MAX_GRASS_TYPES: u32 = 64;
/// Floats per type row (`ed_world::grass::ROW_FLOATS`).
pub const GRASS_TYPE_FLOATS: usize = 32;
const CLUMP_WORDS: usize = 8;
/// Blade slots and segments per bucket (common.slang `GRASS_SLOTS`, `GRASS_SEGS`).
pub const BUCKETS: [(u32, u32); 5] = [(64, 6), (32, 5), (16, 4), (8, 3), (4, 2)];
pub(crate) const GRASS_BUCKETS: u32 = BUCKETS.len() as u32;
/// Draw lists: the view's buckets, then the nearest cascade's.
pub(crate) const GRASS_LISTS: u32 = 2 * GRASS_BUCKETS;
/// Visible-list slots per list.
pub(crate) const GRASS_LIST_CAP: u32 = 1 << 18;

/// One clump for the grass buffer.
#[derive(Clone, Copy, Debug)]
pub struct GrassClump {
    /// Centre on the ground relative to its node origin (render frame).
    pub pos: Vec3,
    /// Type row.
    pub kind: u32,
    /// Rank 0..1 against the distance keep.
    pub rank: f32,
    /// Ground normal and the facing tangent (render frame, unit).
    pub up: Vec3,
    pub facing: Vec3,
    pub seed: u32,
    /// Height scale (0..4) and dry share (0..1).
    pub height: f32,
    pub dry: f32,
}

/// Distance rules and switches (`FrameData.grass`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrassConfig {
    /// Full blade count inside this distance (m); `(r0/d)^2` of it past.
    pub r0: f32,
    /// Nothing past this distance (m); blades shrink away over its last 40 %.
    pub range: f32,
    /// Fewest blades per clump before clumps thin.
    pub bmin: f32,
    /// Most blade widening as blades thin (coverage holds).
    pub width_max: f32,
    /// Blades cast into the nearest shadow cascade.
    pub shadows: bool,
    /// Shading normal bent towards the ground normal (0 blade, 1 ground) near; far goes to 1.
    pub up_blend: f32,
    /// Wind moves the blades.
    pub wind: bool,
}

impl Default for GrassConfig {
    fn default() -> Self {
        Self { r0: 14.0, range: 110.0, bmin: 5.0, width_max: 5.0, shadows: true, up_blend: 0.3, wind: true }
    }
}

/// A block of clump slots, freed as one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrassRange {
    pub start: u32,
    pub len: u32,
}

/// The bucket index buffer: per bucket, `slots` blade strips of `segs` segments (two triangles
/// each), vertex `blade * 2 * (segs + 1) + 2 * seg + side`. Returns the indices and each
/// bucket's first index.
pub fn bucket_indices() -> (Vec<u32>, [u32; BUCKETS.len()]) {
    let mut idx = Vec::new();
    let mut first = [0u32; BUCKETS.len()];
    for (b, &(slots, segs)) in BUCKETS.iter().enumerate() {
        first[b] = idx.len() as u32;
        let vpb = 2 * (segs + 1);
        for k in 0..slots {
            for s in 0..segs {
                let v = k * vpb + 2 * s;
                idx.extend_from_slice(&[v, v + 1, v + 2, v + 2, v + 1, v + 3]);
            }
        }
    }
    (idx, first)
}

pub(crate) struct GrassStore {
    pub clumps: Option<Tracked>,
    pub types: Option<Tracked>,
    pub index: Option<Tracked>,
    alloc: RangeAlloc,
    type_count: u32,
    pub config: Option<GrassConfig>,
    pub on: bool,
}

impl GrassStore {
    pub fn new() -> Self {
        Self { clumps: None, types: None, index: None, alloc: RangeAlloc::new(MAX_GRASS_CLUMPS), type_count: 0, config: None, on: true }
    }

    fn ensure(&mut self, dev: &dyn Device) -> Result<(), ed_gpu::Error> {
        if self.clumps.is_some() {
            return Ok(());
        }
        let u = Usage { shader_read: true, copy_dst: true, ..Default::default() };
        let mut clumps = Tracked::new(dev, "grass clumps", MAX_GRASS_CLUMPS as u64 * (CLUMP_WORDS * 4) as u64, u)?;
        dev.write_buffer(clumps.id, 0, &vec![0u8; MAX_GRASS_CLUMPS as usize * CLUMP_WORDS * 4])?;
        clumps.state = ResourceState::CopyDst;
        let types = Tracked::new(dev, "grass types", MAX_GRASS_TYPES as u64 * (GRASS_TYPE_FLOATS * 4) as u64, u)?;
        let (idx, _) = bucket_indices();
        let mut index = Tracked::new(dev, "grass indices", idx.len() as u64 * 4, u)?;
        dev.write_buffer(index.id, 0, &gpu::bytes(&idx))?;
        index.state = ResourceState::CopyDst;
        self.clumps = Some(clumps);
        self.types = Some(types);
        self.index = Some(index);
        Ok(())
    }

    /// Replaces the type table.
    pub fn set_types(&mut self, dev: &dyn Device, rows: &[[f32; GRASS_TYPE_FLOATS]]) -> Result<(), ed_gpu::Error> {
        self.ensure(dev)?;
        if rows.len() as u32 > MAX_GRASS_TYPES {
            return Err(ed_gpu::Error::Invalid("grass type table full".into()));
        }
        let w: Vec<u32> = rows.iter().flat_map(|r| r.iter().map(|x| x.to_bits())).collect();
        let t = self.types.as_mut().expect("ensured");
        dev.write_buffer(t.id, 0, &gpu::bytes(&w))?;
        t.state = ResourceState::CopyDst;
        self.type_count = rows.len() as u32;
        Ok(())
    }

    pub fn add_clumps(&mut self, dev: &dyn Device, node: NodeSlot, clumps: &[GrassClump]) -> Result<Option<GrassRange>, ed_gpu::Error> {
        self.ensure(dev)?;
        let n = clumps.len() as u32;
        if n == 0 {
            return Ok(None);
        }
        let Some(start) = self.alloc.alloc(n) else { return Ok(None) };
        let mut w = Vec::with_capacity(clumps.len() * CLUMP_WORDS);
        for c in clumps {
            let rank = (c.rank.clamp(0.0, 1.0) * 255.0).round() as u32;
            let h = ((c.height / 4.0).clamp(1.0 / 65535.0, 1.0) * 65535.0).round() as u32;
            let dry = (c.dry.clamp(0.0, 1.0) * 255.0).round() as u32;
            w.extend_from_slice(&[
                c.pos.x.to_bits(),
                c.pos.y.to_bits(),
                c.pos.z.to_bits(),
                (node.0 as u32 & 0xFFF) | ((c.kind & 0xFF) << 12) | (rank << 24),
                gpu::oct_encode(c.up),
                gpu::oct_encode(c.facing),
                c.seed,
                h | (dry << 16),
            ]);
        }
        let tb = self.clumps.as_ref().expect("ensured");
        dev.write_buffer(tb.id, start as u64 * (CLUMP_WORDS * 4) as u64, &gpu::bytes(&w))?;
        Ok(Some(GrassRange { start, len: n }))
    }

    /// Kills the range's clumps at once (height 0); the slots are reused after frames in flight.
    pub fn kill(&mut self, dev: &dyn Device, r: GrassRange) -> Result<(), ed_gpu::Error> {
        if let Some(tb) = &self.clumps {
            dev.write_buffer(tb.id, r.start as u64 * (CLUMP_WORDS * 4) as u64, &vec![0u8; r.len as usize * CLUMP_WORDS * 4])?;
        }
        Ok(())
    }
    pub fn free(&mut self, r: GrassRange) {
        self.alloc.free(r.start, r.len);
    }

    /// Slots the cull walks (high-water mark), 0 when nothing draws.
    pub fn slots(&self) -> u32 {
        if self.on && self.config.is_some() && self.clumps.is_some() && self.type_count > 0 { self.alloc.high_water() } else { 0 }
    }
    pub fn used(&self) -> u32 {
        self.alloc.used()
    }

    /// `FrameData.grass` words.
    pub fn words(&self) -> [u32; GRASS_WORDS] {
        let mut w = [0u32; GRASS_WORDS];
        let Some(c) = self.config else { return w };
        if let (Some(t), Some(k)) = (&self.clumps, &self.types) {
            w[0] = t.bindless.0;
            w[1] = k.bindless.0;
        }
        w[2] = self.slots();
        w[3] = if w[2] > 0 { 1 | if c.shadows { 2 } else { 0 } | if c.wind { 4 } else { 0 } } else { 0 };
        let f = [c.r0, c.range, c.bmin, c.width_max, c.up_blend, 0.0, 0.0, 0.0];
        for (i, x) in f.iter().enumerate() {
            w[4 + i] = x.to_bits();
        }
        w
    }

    pub fn destroy(self, dev: &dyn Device) {
        for t in [self.clumps, self.types, self.index].into_iter().flatten() {
            t.destroy(dev);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_offsets_match_the_shader_table() {
        let (idx, first) = bucket_indices();
        // common.slang GRASS_FIRST mirrors these.
        assert_eq!(first, [0, 2304, 3264, 3648, 3792]);
        assert_eq!(idx.len(), 3792 + 4 * 2 * 6);
        // Every bucket's strip stays inside its vertex count.
        for (b, &(slots, segs)) in BUCKETS.iter().enumerate() {
            let end = if b + 1 < BUCKETS.len() { first[b + 1] } else { idx.len() as u32 } as usize;
            let max = *idx[first[b] as usize..end].iter().max().unwrap();
            assert_eq!(max, slots * 2 * (segs + 1) - 1);
        }
    }
}
