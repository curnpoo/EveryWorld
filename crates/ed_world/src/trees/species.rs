//! Species as data (step 1 of `docs/plans/procedural-flora.md`).
//!
//! A [`SpeciesDef`] is one row: leaf palette, far-LOD card budget, trunk
//! collider and the ranges a [`Genome`] may vary within. New species are new
//! rows (plus a generator in `trees/mod.rs` until generators are parametric).
//! A [`Genome`] is one tree's own draw from those ranges; the default genome
//! is the identity, so the cached shared models look exactly as before.

use super::WindMesh;
use crate::foliage::TreeKind;
use glam::Vec3;

/// Trunk collision cylinder at instance scale 1 (metres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trunk {
    pub radius: f32,
    pub height: f32,
}

/// Ranges one species' genomes are drawn from (min, max).
#[derive(Debug, Clone, Copy)]
pub struct Ranges {
    /// Overall height multiplier.
    pub height: (f32, f32),
    /// Crown/trunk width multiplier.
    pub width: (f32, f32),
    /// Brightness multiplier on leaves.
    pub leaf_value: (f32, f32),
    /// Hue wobble on leaves (multiplies the green channel vs red).
    pub leaf_hue: (f32, f32),
}

pub const STANDARD: Ranges = Ranges { height: (0.8, 1.25), width: (0.82, 1.2), leaf_value: (0.88, 1.12), leaf_hue: (0.94, 1.06) };
/// Shrubs and ground plants vary more.
pub const LOOSE: Ranges = Ranges { height: (0.65, 1.4), width: (0.7, 1.35), leaf_value: (0.85, 1.15), leaf_hue: (0.92, 1.08) };

#[derive(Debug, Clone, Copy)]
pub struct SpeciesDef {
    pub kind: TreeKind,
    pub name: &'static str,
    /// Leaf colour (sRGB hex) per seeded variant.
    pub leaf: [u32; 3],
    /// Cards kept in the far-LOD model.
    pub far_cards: usize,
    /// `None`: no collision (shrubs, ferns, grass, small succulents).
    pub trunk: Option<Trunk>,
    pub ranges: Ranges,
}

const fn t(radius: f32, height: f32) -> Option<Trunk> {
    Some(Trunk { radius, height })
}

const fn row(kind: TreeKind, name: &'static str, leaf: [u32; 3], far_cards: usize, trunk: Option<Trunk>, ranges: Ranges) -> SpeciesDef {
    SpeciesDef { kind, name, leaf, far_cards, trunk, ranges }
}

use TreeKind as K;

/// One row per [`TreeKind`], in `TreeKind` declaration order.
pub static SPECIES: [SpeciesDef; 23] = [
    row(K::Broadleaf, "oak", [0x5d8a3a, 0x6b9442, 0x547f36], 34, t(0.40, 4.0), STANDARD),
    row(K::Conifer, "spruce", [0x2f5a3a, 0x355f3c, 0x2b5236], 34, t(0.28, 4.0), STANDARD),
    row(K::Birch, "birch", [0x86b04a, 0x93b851, 0x7ca545], 34, t(0.16, 4.0), STANDARD),
    row(K::Palm, "coconut palm", [0x6d9a3c, 0x78a443, 0x669236], 8, t(0.20, 4.0), STANDARD),
    row(K::Acacia, "acacia", [0x7c8e40, 0x879845, 0x72863c], 34, t(0.30, 4.0), STANDARD),
    row(K::Cypress, "cypress", [0x2f4a2c, 0x344f30, 0x2b4428], 34, t(0.28, 4.0), STANDARD),
    row(K::Shrub, "shrub", [0x5f8a40, 0x6b9446, 0x587f3a], 34, None, LOOSE),
    row(K::Cactus, "saguaro", [0x000000; 3], 0, t(0.35, 4.0), STANDARD),
    row(K::Maple, "maple", [0x6c9a3e, 0x7aa344, 0x8a9a3a], 34, t(0.34, 4.0), STANDARD),
    row(K::PlaneTree, "plane", [0x6f9a45, 0x7ba34c, 0x66903f], 34, t(0.40, 4.0), STANDARD),
    row(K::Fir, "fir", [0x2c5040, 0x2a4a3e, 0x33584a], 34, t(0.28, 4.0), STANDARD),
    row(K::Pine, "pine", [0x4a6e3c, 0x557842, 0x46683a], 34, t(0.28, 4.0), STANDARD),
    row(K::DatePalm, "date palm", [0x6f8a58, 0x7a9260, 0x688252], 8, t(0.24, 4.0), STANDARD),
    row(K::Olive, "olive", [0x7d8c62, 0x86946a, 0x74845c], 34, t(0.26, 3.0), STANDARD),
    row(K::Baobab, "baobab", [0x6f9440, 0x7a9c46, 0x66893a], 34, t(1.80, 4.0), STANDARD),
    row(K::Banana, "banana", [0x7aa83e, 0x86b244, 0x70a03a], 8, t(0.15, 2.5), LOOSE),
    row(K::PricklyPear, "prickly pear", [0x000000; 3], 0, t(0.45, 1.0), LOOSE),
    row(K::Boxwood, "boxwood", [0x3f6a2e, 0x467333, 0x3a6229], 34, None, LOOSE),
    row(K::Juniper, "juniper", [0x4a6f55, 0x52775a, 0x45684e], 34, None, LOOSE),
    row(K::Oleander, "oleander", [0x557a3e, 0x5e8244, 0x507438], 34, None, LOOSE),
    row(K::Fern, "fern", [0x5e9a3a, 0x68a240, 0x569034], 8, None, LOOSE),
    row(K::GrassTussock, "tussock", [0x9aa457, 0x8e9c4e, 0xa6a860], 8, None, LOOSE),
    row(K::FloweringBush, "flowering bush", [0x5a8a40, 0x628f44, 0x55823c], 34, None, LOOSE),
];

/// The species row for `kind`. Rows are in `TreeKind` order (tested).
pub fn def(kind: TreeKind) -> &'static SpeciesDef {
    &SPECIES[kind as usize]
}

/// Environment an individual grew in; shifts the genome draw.
#[derive(Debug, Clone, Copy)]
pub struct Conditions {
    /// 0.5 (stunted: cold, dry, high) .. 1.2 (lush); see `foliage::vigor`.
    pub vigor: f32,
    /// 0 (open ground) .. 1 (dense stand): crowded trees grow taller and narrower.
    pub crowding: f32,
}

impl Default for Conditions {
    fn default() -> Self {
        Self { vigor: 1.0, crowding: 0.0 }
    }
}

/// One tree's own parameters. `Genome::default()` is the identity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Genome {
    pub height: f32,
    pub width: f32,
    /// Linear-colour multiplier on leaf vertices (r, g, b).
    pub leaf_mul: [f32; 3],
}

impl Default for Genome {
    fn default() -> Self {
        Self { height: 1.0, width: 1.0, leaf_mul: [1.0; 3] }
    }
}

impl Genome {
    /// Deterministic draw for `kind` from `seed` (hash of position/tile),
    /// shifted by the growing conditions. Same inputs, same tree.
    pub fn sample(kind: TreeKind, seed: u64, c: Conditions) -> Genome {
        let r = &def(kind).ranges;
        let h = |salt: u64| crate::hash01(seed, 0x6E40 + salt);
        let lerp = |(a, b): (f32, f32), u: f32| a + (b - a) * u;
        // Poor sites stunt height more than width; crowding does the reverse.
        let vig = c.vigor.clamp(0.5, 1.2);
        let height = lerp(r.height, h(1)) * (0.75 + 0.25 * vig) * (1.0 + 0.12 * c.crowding);
        let width = lerp(r.width, h(2)) * (0.85 + 0.15 * vig) * (1.0 - 0.18 * c.crowding);
        let value = lerp(r.leaf_value, h(3)) * (0.9 + 0.1 * vig);
        let hue = lerp(r.leaf_hue, h(4));
        Genome { height, width, leaf_mul: [value / hue, value, value / hue.sqrt()] }
    }

    pub fn is_identity(&self) -> bool {
        *self == Genome::default()
    }

    /// Reshape a grown model: non-uniform scale (width on x/z, height on y)
    /// and a leaf tint. Wind sway amplitudes are left as grown.
    pub(super) fn apply(&self, m: &mut super::TreeModel) {
        if self.is_identity() {
            return;
        }
        for w in [&mut m.bark, &mut m.leaves] {
            scale_xyz(w, self.width, self.height);
        }
        for c in &mut m.leaves.mesh.colors {
            for (ch, k) in c.iter_mut().zip(self.leaf_mul) {
                *ch = (*ch * k).min(1.0);
            }
        }
        m.height *= self.height;
        m.crown_radius *= self.width;
    }
}

fn scale_xyz(w: &mut WindMesh, wide: f32, tall: f32) {
    for p in &mut w.mesh.positions {
        p[0] *= wide;
        p[1] *= tall;
        p[2] *= wide;
    }
    // Normals transform by the inverse transpose.
    for n in &mut w.mesh.normals {
        let v = Vec3::new(n[0] / wide, n[1] / tall, n[2] / wide).normalize_or(Vec3::Y);
        *n = v.to_array();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trees::{VARIANTS, build, build_genome};

    #[test]
    fn rows_follow_kind_order() {
        assert_eq!(SPECIES.len(), TreeKind::ALL.len());
        for (i, d) in SPECIES.iter().enumerate() {
            assert_eq!(d.kind as usize, i, "{}", d.name);
        }
    }

    #[test]
    fn default_genome_is_identity() {
        for &k in &[TreeKind::Broadleaf, TreeKind::Conifer, TreeKind::Palm] {
            let a = build(k, 0, false);
            let b = build_genome(k, 0, false, &Genome::default());
            assert_eq!(a.leaves.mesh.positions, b.leaves.mesh.positions);
            assert_eq!(a.bark.mesh.positions, b.bark.mesh.positions);
            assert_eq!(a.leaves.mesh.colors, b.leaves.mesh.colors);
        }
    }

    #[test]
    fn genomes_are_deterministic_and_all_different() {
        let c = Conditions::default();
        assert_eq!(Genome::sample(TreeKind::Broadleaf, 42, c), Genome::sample(TreeKind::Broadleaf, 42, c));
        let mut seen = std::collections::HashSet::new();
        for s in 0..500u64 {
            let g = Genome::sample(TreeKind::Broadleaf, s, c);
            assert!(seen.insert((g.height.to_bits(), g.width.to_bits(), g.leaf_mul[1].to_bits())));
            let r = def(TreeKind::Broadleaf).ranges;
            assert!(g.height >= r.height.0 * 0.7 && g.height <= r.height.1 * 1.2, "{g:?}");
        }
    }

    #[test]
    fn stunted_sites_grow_shorter() {
        let poor = Conditions { vigor: 0.5, crowding: 0.0 };
        let rich = Conditions { vigor: 1.2, crowding: 0.0 };
        let n = 200;
        let mean = |c| (0..n).map(|s| Genome::sample(TreeKind::Pine, s, c).height).sum::<f32>() / n as f32;
        assert!(mean(poor) < mean(rich) * 0.95);
    }

    #[test]
    fn genome_reshapes_model() {
        let base = build(TreeKind::Broadleaf, 1, false);
        let g = Genome { height: 1.3, width: 0.8, leaf_mul: [1.0, 0.9, 1.0] };
        let m = build_genome(TreeKind::Broadleaf, 1, false, &g);
        assert!((m.height - base.height * 1.3).abs() < 1e-3);
        assert_eq!(m.leaves.mesh.positions.len(), base.leaves.mesh.positions.len());
        for n in &m.leaves.mesh.normals {
            assert!((Vec3::from(*n).length() - 1.0).abs() < 1e-3);
        }
        let _ = VARIANTS;
    }

    #[test]
    fn trunk_colliders_only_on_real_trees() {
        for &k in TreeKind::TREES {
            assert!(def(k).trunk.is_some(), "{k:?} needs a trunk collider");
        }
        for &k in TreeKind::SHRUBS.iter().filter(|k| !TreeKind::TREES.contains(k)) {
            assert!(def(k).trunk.is_none(), "{k:?}");
        }
    }
}
