//! M3 test content: a textured quad and a two-joint skinned bar, and pack -> renderer texture
//! conversion.
#![allow(dead_code)]

use ed_assets::texture::{self, Rgba8, Role};
use ed_pack::AssetId;
use ed_pack::kinds::TextureFormat;
use ed_render::scene::{MeshAsset, MeshLod, SkinnedMeshAsset, TextureAsset};
use ed_gpu::Format;

/// Bakes an RGBA8 image through the real asset pipeline (BC7 / BC5 with mips) and converts it.
pub fn baked(img: &Rgba8, role: Role) -> TextureAsset {
    let t = texture::bake(img, role, &AssetId::from_path("test/tex")).unwrap();
    let format = match (t.format, role) {
        (TextureFormat::Bc7, Role::Colour) => Format::Bc7Srgb,
        (TextureFormat::Bc7, _) => Format::Bc7Unorm,
        (TextureFormat::Bc5, _) => Format::Bc5Unorm,
        (TextureFormat::Bc4, _) => Format::Bc4Unorm,
        (f, _) => panic!("unexpected format {f:?}"),
    };
    TextureAsset { format, width: t.width, height: t.height, mips: t.mips.iter().map(|m| m.to_vec()).collect() }
}

/// A solid-colour image.
pub fn solid(w: u32, rgba: [u8; 4]) -> Rgba8 {
    Rgba8 { width: w, height: w, data: rgba.repeat((w * w) as usize) }
}

/// A 2 x 2 quadrant image (top-left, top-right, bottom-left, bottom-right colours).
pub fn quadrants(w: u32, c: [[u8; 4]; 4]) -> Rgba8 {
    let mut data = Vec::with_capacity((w * w * 4) as usize);
    for y in 0..w {
        for x in 0..w {
            let q = (y >= w / 2) as usize * 2 + (x >= w / 2) as usize;
            data.extend_from_slice(&c[q]);
        }
    }
    Rgba8 { width: w, height: w, data }
}

/// A vertical quad, 1 x 1, facing +Z, UV (0,0) top-left.
pub fn quad() -> MeshAsset {
    MeshAsset {
        positions: vec![[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.5, 0.5, 0.0], [-0.5, 0.5, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uvs: vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        lods: vec![MeshLod { indices: vec![0, 1, 2, 0, 2, 3], error: 0.0 }],
    }
}

/// A 0.4 x 2 x 0.4 bar standing on y = 0, 4 rings; joint 0 at the base, joint 1 at y = 1,
/// weights blend linearly up the bar.
pub fn skinned_bar() -> SkinnedMeshAsset {
    let mut m = MeshAsset::default();
    let (mut joints, mut weights) = (Vec::new(), Vec::new());
    let rings = 5;
    let corners = [(-0.2, -0.2), (0.2, -0.2), (0.2, 0.2), (-0.2, 0.2)];
    for r in 0..rings {
        let y = 2.0 * r as f32 / (rings - 1) as f32;
        for (x, z) in corners {
            m.positions.push([x, y, z]);
            let n = glam::Vec3::new(x, 0.0, z).normalize();
            m.normals.push(n.to_array());
            m.uvs.push([0.0, 0.0]);
            let t = (y / 2.0).clamp(0.0, 1.0);
            joints.push([0, 1, 0, 0]);
            weights.push([1.0 - t, t, 0.0, 0.0]);
        }
    }
    let mut idx = Vec::new();
    for r in 0..rings - 1 {
        for c in 0..4u32 {
            let (a, b) = (r * 4 + c, r * 4 + (c + 1) % 4);
            let (a2, b2) = (a + 4, b + 4);
            idx.extend_from_slice(&[a, b, b2, a, b2, a2]);
        }
    }
    // Caps.
    let top = (rings - 1) * 4;
    idx.extend_from_slice(&[top, top + 3, top + 2, top, top + 2, top + 1]);
    idx.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    m.lods.push(MeshLod { indices: idx, error: 0.0 });
    SkinnedMeshAsset { mesh: m, joints, weights, joint_count: 2, pose_margin: 2.0 }
}
