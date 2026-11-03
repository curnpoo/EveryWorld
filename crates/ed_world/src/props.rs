//! Procedural low-poly geometry for trees, rocks and street furniture, and
//! baking instance lists into one merged mesh per tile (one draw call for
//! thousands of trees). Stylized on purpose: chunky shapes, flat-ish
//! shading, readable silhouettes.

use crate::foliage::{RockInstance, TreeInstance, TreeKind};
use crate::mesh::{MeshData, rgb, scale_color};
use crate::roads::{PoleKind, PropKind, SignalLayout, StreetProp};
use glam::{Quat, Vec3};
use std::f32::consts::TAU;

fn cylinder(m: &mut MeshData, base: Vec3, r0: f32, r1: f32, h: f32, seg: usize, c: [f32; 4]) {
    for i in 0..seg {
        let a0 = i as f32 / seg as f32 * TAU;
        let a1 = (i + 1) as f32 / seg as f32 * TAU;
        let d0 = Vec3::new(a0.cos(), 0.0, a0.sin());
        let d1 = Vec3::new(a1.cos(), 0.0, a1.sin());
        let p = [base + d0 * r0, base + d1 * r0, base + d1 * r1 + Vec3::Y * h, base + d0 * r1 + Vec3::Y * h];
        // Outward-facing: order so normal points along d.
        m.quad([p[1], p[0], p[3], p[2]], [[0.0; 2]; 4], c);
    }
    if r1 > 0.001 {
        // Cap the top.
        let top = base + Vec3::Y * h;
        for i in 0..seg {
            let a0 = i as f32 / seg as f32 * TAU;
            let a1 = (i + 1) as f32 / seg as f32 * TAU;
            m.triangle([top, top + Vec3::new(a1.cos(), 0.0, a1.sin()) * r1, top + Vec3::new(a0.cos(), 0.0, a0.sin()) * r1], [[0.0; 2]; 3], c);
        }
    }
}

fn cone(m: &mut MeshData, base: Vec3, r: f32, h: f32, seg: usize, c: [f32; 4]) {
    let apex = base + Vec3::Y * h;
    for i in 0..seg {
        let a0 = i as f32 / seg as f32 * TAU;
        let a1 = (i + 1) as f32 / seg as f32 * TAU;
        let p0 = base + Vec3::new(a0.cos(), 0.0, a0.sin()) * r;
        let p1 = base + Vec3::new(a1.cos(), 0.0, a1.sin()) * r;
        m.triangle([p1, p0, apex], [[0.0; 2]; 3], c);
        m.triangle([p0, p1, base], [[0.0; 2]; 3], scale_color(c, 0.7));
    }
}

/// Smooth-shaded low-poly ellipsoid (UV sphere, few segments).
fn blob(m: &mut MeshData, center: Vec3, radii: Vec3, c: [f32; 4]) {
    let (rings, seg) = (5, 8);
    let base = m.positions.len() as u32;
    for j in 0..=rings {
        let phi = j as f32 / rings as f32 * std::f32::consts::PI;
        for i in 0..=seg {
            let th = i as f32 / seg as f32 * TAU;
            let n = Vec3::new(phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin());
            // Darker underside = cheap ambient occlusion.
            let shade = 0.72 + 0.28 * (n.y * 0.5 + 0.5);
            m.vertex((center + n * radii).to_array(), (n / radii).normalize().to_array(), [0.0, 0.0], scale_color(c, shade));
        }
    }
    let w = seg as u32 + 1;
    for j in 0..rings as u32 {
        for i in 0..seg as u32 {
            let a = base + j * w + i;
            let b = a + w;
            m.tri(a, a + 1, b);
            m.tri(a + 1, b + 1, b);
        }
    }
}

/// Unit-scale tree model (≈ 8-14 m tall at scale 1).
pub fn tree_mesh(kind: TreeKind, leaf: [f32; 4]) -> MeshData {
    let mut m = MeshData::default();
    let bark = rgb(0x6b4f3a);
    match kind {
        TreeKind::Broadleaf => {
            cylinder(&mut m, Vec3::ZERO, 0.35, 0.25, 4.0, 5, bark);
            blob(&mut m, Vec3::new(0.0, 6.5, 0.0), Vec3::new(3.6, 3.2, 3.6), leaf);
            blob(&mut m, Vec3::new(1.4, 7.8, 0.6), Vec3::splat(2.2), scale_color(leaf, 1.08));
        }
        TreeKind::Conifer => {
            cylinder(&mut m, Vec3::ZERO, 0.3, 0.2, 2.5, 5, bark);
            for (k, (y, r, h)) in [(1.5, 3.2, 5.0), (4.2, 2.5, 4.5), (6.8, 1.7, 4.2)].iter().enumerate() {
                cone(&mut m, Vec3::new(0.0, *y, 0.0), *r, *h, 7, scale_color(leaf, 0.9 + 0.08 * k as f32));
            }
        }
        TreeKind::Birch => {
            cylinder(&mut m, Vec3::ZERO, 0.2, 0.14, 6.0, 5, rgb(0xe8e4dc));
            blob(&mut m, Vec3::new(0.0, 7.5, 0.0), Vec3::new(2.0, 3.4, 2.0), leaf);
        }
        TreeKind::Palm => {
            // Gently curved trunk + drooping fronds.
            let mut p = Vec3::ZERO;
            for k in 0..5 {
                let next = p + Vec3::new(0.25, 2.0, 0.0);
                cylinder(&mut m, p, 0.28 - k as f32 * 0.03, 0.25 - k as f32 * 0.03, 2.05, 6, rgb(0x8a6e4f));
                p = next;
            }
            for i in 0..7 {
                let a = i as f32 / 7.0 * TAU;
                let d = Vec3::new(a.cos(), 0.0, a.sin());
                let s = d.cross(Vec3::Y) * 0.6;
                let mid = p + d * 2.4 + Vec3::Y * 0.6;
                let tip = p + d * 4.2 - Vec3::Y * 1.2;
                m.triangle([p, mid + s, mid - s], [[0.0; 2]; 3], leaf);
                m.triangle([p, mid - s, mid + s], [[0.0; 2]; 3], leaf);
                m.triangle([mid - s, mid + s, tip], [[0.0; 2]; 3], scale_color(leaf, 0.9));
                m.triangle([mid + s, mid - s, tip], [[0.0; 2]; 3], scale_color(leaf, 0.9));
            }
        }
        TreeKind::Acacia => {
            cylinder(&mut m, Vec3::ZERO, 0.3, 0.2, 4.5, 5, bark);
            blob(&mut m, Vec3::new(0.0, 5.5, 0.0), Vec3::new(4.2, 1.1, 4.2), leaf);
        }
        TreeKind::Cypress => {
            cylinder(&mut m, Vec3::ZERO, 0.2, 0.15, 1.2, 5, bark);
            blob(&mut m, Vec3::new(0.0, 6.0, 0.0), Vec3::new(1.3, 5.5, 1.3), leaf);
        }
        TreeKind::Shrub => {
            blob(&mut m, Vec3::new(0.0, 0.7, 0.0), Vec3::new(1.4, 1.0, 1.4), leaf);
        }
        TreeKind::Cactus => {
            let g = rgb(0x5f8a54);
            cylinder(&mut m, Vec3::ZERO, 0.35, 0.35, 4.0, 7, g);
            cylinder(&mut m, Vec3::new(0.35, 1.6, 0.0), 0.22, 0.22, 0.4, 6, g);
            cylinder(&mut m, Vec3::new(0.75, 1.6, 0.0), 0.22, 0.22, 1.5, 6, g);
            cylinder(&mut m, Vec3::new(-0.35, 2.2, 0.0), 0.2, 0.2, 0.35, 6, g);
            cylinder(&mut m, Vec3::new(-0.7, 2.2, 0.0), 0.2, 0.2, 1.2, 6, g);
        }
    }
    m
}

fn rock_mesh(c: [f32; 4]) -> MeshData {
    // Squashed, faceted blob reads as "rock".
    let mut m = MeshData::default();
    let (rings, seg) = (3, 6);
    let base = m.positions.len() as u32;
    for j in 0..=rings {
        let phi = j as f32 / rings as f32 * std::f32::consts::PI;
        for i in 0..=seg {
            let th = i as f32 / seg as f32 * TAU;
            let n = Vec3::new(phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin());
            let jitter = 0.85 + 0.3 * crate::hash01(i as u64 % seg as u64, j as u64);
            m.vertex((n * jitter).to_array(), n.to_array(), [0.0; 2], c);
        }
    }
    let w = seg as u32 + 1;
    for j in 0..rings as u32 {
        for i in 0..seg as u32 {
            let a = base + j * w + i;
            let b = a + w;
            m.tri(a, a + 1, b);
            m.tri(a + 1, b + 1, b);
        }
    }
    m
}

/// Rotate a model so its +Y aligns with `up`, spin about it, scale, place.
fn place(dst: &mut MeshData, src: &MeshData, pos: Vec3, up: Vec3, yaw: f32, scale: Vec3, tint: Option<[f32; 4]>) {
    let rot = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(yaw);
    let base = dst.positions.len() as u32;
    for i in 0..src.positions.len() {
        let p = rot * (Vec3::from(src.positions[i]) * scale) + pos;
        let n = (rot * (Vec3::from(src.normals[i]) / scale)).normalize_or_zero();
        let c = match tint {
            Some(t) => [src.colors[i][0] * t[0], src.colors[i][1] * t[1], src.colors[i][2] * t[2], 1.0],
            None => src.colors[i],
        };
        dst.vertex(p.to_array(), n.to_array(), src.uvs[i], c);
    }
    dst.indices.extend(src.indices.iter().map(|i| i + base));
}

/// Merge all trees of a tile into one mesh.
pub fn bake_trees(trees: &[TreeInstance], up: Vec3) -> MeshData {
    use std::collections::HashMap;
    let mut cache: HashMap<TreeKind, MeshData> = HashMap::new();
    let mut out = MeshData::default();
    for t in trees {
        // Model uses white leaves; per-instance tint colors them.
        let model = cache.entry(t.kind).or_insert_with(|| tree_mesh(t.kind, [1.0; 4]));
        let leaf_tinted: MeshData;
        let src = if t.kind == TreeKind::Cactus {
            model
        } else {
            // Tint only near-white (leaf) vertices; keep bark color.
            let mut m = model.clone();
            for c in &mut m.colors {
                if c[0] > 0.6 && c[1] > 0.6 && c[2] > 0.6 && (c[0] - c[2]).abs() < 0.01 {
                    let k = c[0];
                    *c = [t.tint[0] * k, t.tint[1] * k, t.tint[2] * k, 1.0];
                }
            }
            leaf_tinted = m;
            &leaf_tinted
        };
        place(&mut out, src, t.pos, up, t.yaw, Vec3::splat(t.scale), None);
    }
    out
}

pub fn bake_rocks(rocks: &[RockInstance], up: Vec3) -> MeshData {
    let model = rock_mesh([1.0; 4]);
    let mut out = MeshData::default();
    for r in rocks {
        place(&mut out, &model, r.pos, up, r.yaw, r.scale, Some(r.tint));
    }
    out
}

fn street_prop_mesh(kind: PropKind) -> MeshData {
    let mut m = MeshData::default();
    let grey = rgb(0x5a5f66);
    match kind {
        PropKind::StreetLight => {
            cylinder(&mut m, Vec3::ZERO, 0.12, 0.08, 7.5, 6, grey);
            // Arm out toward the road (+Z in model space) and a lamp head.
            m.quad([Vec3::new(-0.05, 7.4, 0.0), Vec3::new(0.05, 7.4, 0.0), Vec3::new(0.05, 7.5, 2.0), Vec3::new(-0.05, 7.5, 2.0)], [[0.0; 2]; 4], grey);
            blob(&mut m, Vec3::new(0.0, 7.35, 2.0), Vec3::new(0.25, 0.12, 0.45), rgb(0xfff2c6));
        }
        PropKind::UtilityPole(kind) => {
            let c = if kind == PoleKind::Wood { rgb(0x6e5440) } else { rgb(0xb5b2aa) };
            cylinder(&mut m, Vec3::ZERO, 0.16, 0.12, 10.0, 6, c);
            m.quad([Vec3::new(-1.2, 9.2, -0.06), Vec3::new(1.2, 9.2, -0.06), Vec3::new(1.2, 9.4, -0.06), Vec3::new(-1.2, 9.4, -0.06)], [[0.0; 2]; 4], c);
            m.quad([Vec3::new(1.2, 9.2, 0.06), Vec3::new(-1.2, 9.2, 0.06), Vec3::new(-1.2, 9.4, 0.06), Vec3::new(1.2, 9.4, 0.06)], [[0.0; 2]; 4], c);
            if kind == PoleKind::Wood {
                cylinder(&mut m, Vec3::new(0.0, 7.8, 0.3), 0.3, 0.3, 0.9, 6, rgb(0x777b80)); // transformer can
            }
        }
        PropKind::TrafficSignal(layout) => {
            let pole = rgb(0x3d4147);
            cylinder(&mut m, Vec3::ZERO, 0.12, 0.1, 5.5, 6, pole);
            let housing = rgb(0x2a2c30);
            let lights = [rgb(0xff3b30), rgb(0xffcc00), rgb(0x34c759)];
            for (k, c) in lights.iter().enumerate() {
                let off = match layout {
                    SignalLayout::Vertical => Vec3::new(0.0, 5.2 - k as f32 * 0.4, 0.25),
                    SignalLayout::Horizontal => Vec3::new(-0.4 + k as f32 * 0.4, 5.0, 0.25),
                };
                blob(&mut m, off - Vec3::Z * 0.1, Vec3::splat(0.22), housing);
                blob(&mut m, off + Vec3::Z * 0.06, Vec3::splat(0.13), *c);
            }
        }
        PropKind::StopSign => {
            cylinder(&mut m, Vec3::ZERO, 0.04, 0.04, 2.2, 5, grey);
            let red = rgb(0xc8102e);
            let c = Vec3::new(0.0, 2.3, 0.05);
            for i in 0..8 {
                let a0 = (i as f32 + 0.5) / 8.0 * TAU;
                let a1 = (i as f32 + 1.5) / 8.0 * TAU;
                let p0 = c + Vec3::new(a0.cos(), a0.sin(), 0.0) * 0.38;
                let p1 = c + Vec3::new(a1.cos(), a1.sin(), 0.0) * 0.38;
                m.triangle([c, p0, p1], [[0.0; 2]; 3], red);
                m.triangle([c, p1, p0], [[0.0; 2]; 3], scale_color(red, 0.6));
            }
        }
    }
    m
}

pub fn bake_street_props(props: &[StreetProp], up: Vec3) -> MeshData {
    use std::collections::HashMap;
    let mut cache: HashMap<PropKind, MeshData> = HashMap::new();
    let mut out = MeshData::default();
    for p in props {
        let model = cache.entry(p.kind).or_insert_with(|| street_prop_mesh(p.kind));
        // Model faces +Z; rotate so +Z points along `facing` (projected flat).
        let f = p.facing.reject_from_normalized(up).normalize_or(Vec3::Z);
        let rot_up = Quat::from_rotation_arc(Vec3::Y, up);
        let local_f = rot_up.inverse() * f;
        let yaw = local_f.x.atan2(local_f.z);
        place(&mut out, model, p.pos, up, yaw, Vec3::ONE, None);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tree_kind_builds() {
        for k in [TreeKind::Broadleaf, TreeKind::Conifer, TreeKind::Birch, TreeKind::Palm, TreeKind::Acacia, TreeKind::Cypress, TreeKind::Shrub, TreeKind::Cactus] {
            let m = tree_mesh(k, [0.3, 0.6, 0.2, 1.0]);
            assert!(m.triangle_count() > 10 && m.triangle_count() < 800, "{k:?}: {}", m.triangle_count());
        }
    }
}
