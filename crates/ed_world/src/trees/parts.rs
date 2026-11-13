//! Authored tree parts (trees G6): card assemblies and needle textures made in Blender
//! (`tools/blender/trees/build_tree_parts.py`, baked by `tools/blender/trees/bake_tree_parts.sh`
//! into `<cache>/tree_parts/`), installed once by the app before the first bank mesh is built.
//! The bank assembles Norway spruce and European larch from them (`bank::assembled_conifer`);
//! without them it falls back to the procedural `whorl_conifer`, so tests and machines without
//! the bake still work.
//!
//! * A [`Part`] is a few cards in part space: sprays have +X along the branch towards its tip,
//!   +Y up, +Z to the side, the origin where the spray sits on the branch; leaders stand on +Y.
//!   UVs are cell-local (v = 0 at the attaching end), the cell named per vertex.
//! * Each authored texture fills one atlas cell ([`CellImage`], `textures::leaf_atlas` paints it
//!   in place of the procedural fallback, renormalised to the atlas' neutral green).

use super::textures::LeafCell;
use std::sync::OnceLock;

/// One authored card assembly.
#[derive(Clone, Debug, Default)]
pub struct Part {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Cell-local UV (u across, v = 0 at the attaching end).
    pub uvs: Vec<[f32; 2]>,
    pub cells: Vec<LeafCell>,
    pub indices: Vec<u32>,
    /// Extent along the part's main axis (m): +X for sprays, +Y for leaders.
    pub length: f32,
}

impl Part {
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }
    /// Cards (connected pieces).
    pub fn cards(&self) -> usize {
        let n = self.positions.len();
        let mut p: Vec<usize> = (0..n).collect();
        fn find(p: &mut [usize], mut x: usize) -> usize {
            while p[x] != x {
                p[x] = p[p[x]];
                x = p[x];
            }
            x
        }
        for t in self.indices.chunks_exact(3) {
            let a = find(&mut p, t[0] as usize);
            for &v in &t[1..] {
                let r = find(&mut p, v as usize);
                p[r] = a;
            }
        }
        let mut roots: Vec<usize> = (0..n).map(|i| find(&mut p, i)).collect();
        roots.sort_unstable();
        roots.dedup();
        roots.len()
    }
}

/// An authored card texture: `size` x `size` RGBA8 (sRGB, straight alpha), row 0 at the tip
/// end of the card (the top of the image).
#[derive(Clone, Debug)]
pub struct CellImage {
    pub cell: LeafCell,
    pub size: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct TreeParts {
    pub parts: Vec<Part>,
    pub images: Vec<CellImage>,
    /// Content hash (cache keys of everything baked from the parts).
    pub fingerprint: u64,
}

impl TreeParts {
    pub fn part(&self, name: &str) -> Option<&Part> {
        self.parts.iter().find(|p| p.name == name)
    }
    pub fn image(&self, cell: LeafCell) -> Option<&CellImage> {
        self.images.iter().find(|i| i.cell == cell)
    }
    /// The parts one species needs (`<prefix>_spray_a/b/c`, `_far`, `_leader`), when all are
    /// present.
    pub fn set(&self, prefix: &str) -> Option<PartSet<'_>> {
        let g = |s: &str| self.part(&format!("{prefix}_{s}"));
        Some(PartSet { sprays: [g("spray_a")?, g("spray_b")?, g("spray_c")?], far: g("spray_far")?, leader: g("leader")? })
    }

    /// Fills [`TreeParts::fingerprint`] from the content (FNV-1a over the parts and images).
    pub fn seal(mut self) -> Self {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |b: &[u8]| {
            for x in b {
                h ^= *x as u64;
                h = h.wrapping_mul(0x100_0000_01b3);
            }
        };
        for p in &self.parts {
            eat(p.name.as_bytes());
            for v in &p.positions {
                for c in v {
                    eat(&c.to_le_bytes());
                }
            }
            for v in &p.uvs {
                for c in v {
                    eat(&c.to_le_bytes());
                }
            }
            for c in &p.cells {
                eat(&[*c as u8]);
            }
            for i in &p.indices {
                eat(&i.to_le_bytes());
            }
        }
        for i in &self.images {
            eat(&[i.cell as u8]);
            eat(&i.rgba);
        }
        self.fingerprint = h;
        self
    }
}

/// The parts of one species.
#[derive(Clone, Copy, Debug)]
pub struct PartSet<'a> {
    /// a: full spray with curtains, b: lighter, c: brush (sunlit upper crown).
    pub sprays: [&'a Part; 3],
    /// The T1 stand-in (two cards).
    pub far: &'a Part,
    pub leader: &'a Part,
}

/// The atlas cell an authored texture name fills.
pub fn cell_named(name: &str) -> Option<LeafCell> {
    Some(match name {
        "spruce_spray" => LeafCell::SpruceSpray,
        "spruce_curtain" => LeafCell::SpruceCurtain,
        "larch_shoot" => LeafCell::LarchShoot,
        "spruce_leader" => LeafCell::SpruceLeader,
        _ => return None,
    })
}

static INSTALLED: OnceLock<Option<TreeParts>> = OnceLock::new();

/// Installs the parts (once per process, before the first bank mesh or leaf atlas; later calls
/// are ignored). `None` records that there are none (fallback generators). Returns whether this
/// call installed.
pub fn install(parts: Option<TreeParts>) -> bool {
    INSTALLED.set(parts).is_ok()
}

/// The installed parts, if any.
pub fn get() -> Option<&'static TreeParts> {
    INSTALLED.get().and_then(|p| p.as_ref())
}

/// Fingerprint of the installed parts (0 = none).
pub fn fingerprint() -> u64 {
    get().map_or(0, |p| p.fingerprint)
}
