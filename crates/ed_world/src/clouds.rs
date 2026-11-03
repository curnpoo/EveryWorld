//! Stylized cloud geometry. Three looks to choose from in lookdev:
//! - `Puffy`: storybook cumulus, a cluster of soft spheres with a flat base.
//! - `Towering`: tall cumulus/cumulonimbus for storms you can see coming.
//! - `Stratus`: a flat, lumpy sheet for overcast days.
//! Shading is baked into vertex colors (bright tops, cool blue-grey bellies)
//! so they read well even before volumetrics.

use crate::mesh::{MeshData, lerp_color, rgb};
use glam::Vec3;
use std::f32::consts::{PI, TAU};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudStyle {
    Puffy,
    Towering,
    Stratus,
}

fn puff(m: &mut MeshData, c: Vec3, r: Vec3, base_y: f32, top_y: f32) {
    let (rings, seg) = (8, 14);
    let top = rgb(0xffffff);
    let belly = rgb(0xb9c6d8);
    let start = m.positions.len() as u32;
    for j in 0..=rings {
        let phi = j as f32 / rings as f32 * PI;
        for i in 0..=seg {
            let th = i as f32 / seg as f32 * TAU;
            let n = Vec3::new(phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin());
            let mut p = c + n * r;
            // Flatten everything below the cloud base.
            let mut nn = n;
            if p.y < base_y {
                p.y = base_y + (p.y - base_y) * 0.08;
                nn = Vec3::new(n.x * 0.3, -1.0, n.z * 0.3).normalize();
            }
            let h = ((p.y - base_y) / (top_y - base_y).max(1.0)).clamp(0.0, 1.0);
            let col = lerp_color(belly, top, (h * 1.4).min(1.0));
            m.vertex(p.to_array(), nn.to_array(), [0.0, 0.0], col);
        }
    }
    let w = seg as u32 + 1;
    for j in 0..rings as u32 {
        for i in 0..seg as u32 {
            let a = start + j * w + i;
            let b = a + w;
            m.tri(a, a + 1, b);
            m.tri(a + 1, b + 1, b);
        }
    }
}

/// One cloud, roughly `size` meters across, base at y=0.
pub fn cloud(style: CloudStyle, seed: u64, size: f32) -> MeshData {
    let mut m = MeshData::default();
    let rnd = |k: u64| crate::hash01(seed, k);
    match style {
        CloudStyle::Puffy => {
            let n = 7 + (rnd(0) * 5.0) as u64;
            let top = size * 0.55;
            for k in 0..n {
                let a = rnd(k * 3 + 1) * TAU;
                let d = rnd(k * 3 + 2).sqrt() * size * 0.38;
                let rr = size * (0.16 + 0.14 * rnd(k * 3 + 3)) * (1.0 - d / size);
                let c = Vec3::new(a.cos() * d, rr * 0.55 + size * 0.04 * rnd(k + 50), a.sin() * d * 0.7);
                puff(&mut m, c, Vec3::new(rr * 1.15, rr, rr * 1.05), 0.0, top);
            }
            // A crown on top makes the classic silhouette.
            puff(&mut m, Vec3::new(0.0, size * 0.22, 0.0), Vec3::splat(size * 0.24), 0.0, top);
        }
        CloudStyle::Towering => {
            let top = size * 1.6;
            for k in 0..14u64 {
                let t = k as f32 / 13.0;
                let a = rnd(k * 5 + 1) * TAU;
                let spread = size * 0.35 * (1.0 - t * 0.5);
                let c = Vec3::new(a.cos() * spread * rnd(k + 9), t * size * 1.25, a.sin() * spread * rnd(k + 19));
                let rr = size * (0.28 - 0.1 * t) * (0.8 + 0.4 * rnd(k + 29));
                puff(&mut m, c, Vec3::splat(rr), 0.0, top);
            }
            // Anvil.
            puff(&mut m, Vec3::new(size * 0.2, size * 1.45, 0.0), Vec3::new(size * 0.75, size * 0.12, size * 0.55), 0.0, top);
        }
        CloudStyle::Stratus => {
            for k in 0..18u64 {
                let c = Vec3::new((rnd(k * 2) - 0.5) * size * 1.6, size * 0.03 * rnd(k + 70), (rnd(k * 2 + 1) - 0.5) * size * 1.6);
                let rr = size * (0.18 + 0.1 * rnd(k + 40));
                puff(&mut m, c, Vec3::new(rr * 1.6, rr * 0.22, rr * 1.4), -size * 0.02, size * 0.08);
            }
        }
    }
    m
}
