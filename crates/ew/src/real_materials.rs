//! Real-material overrides for the proving ground (data, not branches).
//!
//! The proving ground's own materials are flat colours and its meshes have no UVs. When an
//! extra pack that holds real surface materials is mounted (the Megascans pack built from
//! `assets-src/megascans.p1.toml`, mounted with `--pack` / `EW_PACKS`), [`RULES`] re-skin
//! chosen placements with those materials: [`select`] maps (part, proving material) to a
//! [`Rule`], and [`crate::content::Content`] then
//! - builds a material from the pack material `megascans/<real>` with the UV scale set to
//!   `1 / tile_m` (UVs are in metres),
//! - generates metric box-projected UVs for the part's mesh, and for parts whose instance
//!   scale is part of their shape (buildings, the ground slab: unit meshes scaled per
//!   instance) makes the UVs follow the nominal instance size (a mesh copy per size class).
//!
//! A rule whose target material is not in a mounted pack is skipped: without the pack nothing
//! changes. Colour palette indices are the ones of `ed_assets::proving::materials`.
//! Surface tile sizes come from the Megascans `scanArea` metadata (the pack does not keep it).

use ed_assets::proving::{PAL, Slot, part_slot};

/// Which placements a rule re-skins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// The ground slab (proving material 0).
    Ground,
    /// Parts of this slot drawn with palette PBR colour `i` (the toon colours stay toon).
    PaletteColour(Slot, usize),
}

#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub target: Target,
    /// Pack material key (`megascans/<key>`).
    pub real: &'static str,
    /// Physical size of one texture repeat, metres (Megascans `scanArea`).
    pub tile_m: f32,
    /// The part is a unit mesh scaled per instance: UVs follow its (nominal) instance scale.
    pub bake_scale: bool,
}

const fn building(colour: usize, real: &'static str) -> Rule {
    Rule { target: Target::PaletteColour(Slot::Building, colour), real, tile_m: 2.0, bake_scale: true }
}

pub static RULES: &[Rule] = &[
    // The slab under the whole city: asphalt (streets are the ground between the blocks).
    Rule { target: Target::Ground, real: "asphalt_road", tile_m: 2.0, bake_scale: true },
    // Facades by palette colour.
    building(0, "red_brick_wall"),
    building(1, "rough_concrete_wall"),
    building(2, "concrete_pavement"),
    building(3, "granite_paving_slabs"),
    building(4, "red_brick_wall"),
    building(5, "rough_concrete_wall"),
    building(6, "red_brick_wall"),
    building(7, "concrete_pavement"),
];

/// Index in [`RULES`] of the rule for a placement of part `part` (name, `ground` for the slab)
/// with proving material index `material`.
pub fn select(part: &str, material: usize) -> Option<usize> {
    let slot = part_slot(part);
    RULES.iter().position(|r| match r.target {
        Target::Ground => part == "ground" && material == 0,
        Target::PaletteColour(s, c) => slot == Some(s) && material == PAL + c * 2,
    })
}

/// Nominal scale classes for UV generation. The renderer's mesh table is small (256 meshes),
/// so buildings (a unit mesh scaled per instance, ~1.7k distinct sizes) cannot each get a mesh
/// copy. UVs are instead generated for the nearest nominal size (horizontal and vertical
/// classes, geometric steps), so the texture stretch of any instance is at most ~25%.
const H_CLASSES: &[f32] = &[12.0, 15.0, 19.0];
const V_CLASSES: &[f32] = &[14.0, 22.0, 36.0, 58.0, 92.0, 120.0];

fn snap(s: f32, classes: &[f32]) -> f32 {
    if s > 150.0 {
        return s; // the ground slab: exact
    }
    *classes.iter().min_by(|a, b| (s / **a).ln().abs().total_cmp(&(s / **b).ln().abs())).unwrap()
}

/// The nominal scale UVs are generated for, from an instance scale.
pub fn nominal(s: [f32; 3]) -> [f32; 3] {
    [snap(s[0], H_CLASSES), snap(s[1], V_CLASSES), snap(s[2], H_CLASSES)]
}

/// Metric box-projection UVs (metres) for `positions` / `normals`: each vertex projects along
/// its normal's dominant axis (top and bottom: x,z; X faces: z,y; Z faces: x,y). Side faces are
/// mapped the way a viewer outside sees them: the image's up is world up (glTF V runs down the
/// image, so v = -y) and its right is the face's right (u along up x normal), so scans with a
/// direction (weathering streaks, stains running down) read the right way and nothing is
/// mirrored.
pub fn box_uvs(positions: &[[f32; 3]], normals: &[[f32; 3]]) -> Vec<[f32; 2]> {
    positions
        .iter()
        .zip(normals)
        .map(|(p, n)| {
            let a = [n[0].abs(), n[1].abs(), n[2].abs()];
            if a[1] >= a[0] && a[1] >= a[2] {
                [p[0], p[2]]
            } else if a[0] >= a[2] {
                [-p[2] * n[0].signum(), -p[1]]
            } else {
                [p[0] * n[2].signum(), -p[1]]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_select_by_part_and_colour() {
        assert_eq!(RULES[select("ground", 0).unwrap()].real, "asphalt_road");
        assert_eq!(RULES[select("tower", PAL).unwrap()].real, "red_brick_wall");
        assert_eq!(RULES[select("tower", PAL + 2).unwrap()].real, "rough_concrete_wall");
        assert!(select("tower", PAL + 1).is_none(), "toon colours stay toon");
        assert!(select("lamp", PAL).is_none(), "props are not re-skinned");
        assert!(select("sphere", 5).is_none());
    }

    #[test]
    fn every_building_colour_has_a_rule() {
        for c in 0..ed_assets::proving::PAL_COLOURS {
            assert!(select("block", PAL + c * 2).is_some(), "colour {c}");
        }
    }

    #[test]
    fn uvs_are_metric_and_quantised_scales_merge() {
        let uv = box_uvs(&[[1.0, 2.0, 3.0], [1.0, 2.0, 3.0], [1.0, 2.0, 3.0]], &[[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]]);
        assert_eq!(uv, vec![[1.0, 3.0], [-3.0, -2.0], [-1.0, -2.0]]);
        // Right on the image is right on the face seen from outside; up is up.
        let right = |n: [f32; 3], d: [f32; 3]| box_uvs(&[d], &[n])[0][0] - box_uvs(&[[0.0; 3]], &[n])[0][0];
        assert!(right([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]) > 0.0, "+X face: right is -Z");
        assert!(right([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]) > 0.0, "-X face: right is +Z");
        assert!(right([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]) > 0.0, "+Z face: right is +X");
        assert!(right([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0]) > 0.0, "-Z face: right is -X");
        assert!(box_uvs(&[[0.0, 1.0, 0.0]], &[[1.0, 0.0, 0.0]])[0][1] < 0.0, "up the wall is up the image (v down)");
        assert_eq!(nominal([15.4, 31.1, 12.6]), [15.0, 36.0, 12.0]);
        assert_eq!(nominal([4000.0, 2.0, 4000.0])[0], 4000.0);
    }
}
