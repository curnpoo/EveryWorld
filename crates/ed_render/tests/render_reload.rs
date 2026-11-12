//! Hot-reload leak test (M3): 50 reloads of a material, a texture, a mesh and every shader,
//! headless, with frames in flight. Device memory and geometry ranges must stay flat. Its own
//! test binary because `allocated_bytes` is per Metal device, shared by every device in a
//! process.
#![cfg(target_os = "macos")]

mod common;
use common::m3::*;
use common::*;
use ed_assets::texture::Role;
use ed_geo::ObjectId;
use ed_render::scene::*;
use ed_render::shaders::ShaderSources;
use ed_render::*;
use glam::{Affine3A, Vec3};

#[test]
fn fifty_reloads_keep_memory_flat() {
    let Some(mut rig) = Rig::new() else { return };
    let cam = TestScene::test_camera();
    let bc = rig.dev.caps().bc_textures;
    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    let mesh = r.add_mesh(dev, &quad()).unwrap();
    let mat = r.add_material(Material::pbr(Vec3::ONE, 0.0, 0.6)).unwrap();
    let node = NodeSlot(0);
    r.instances().add(&InstanceDesc { node, transform: Affine3A::from_translation(Vec3::new(3.0, 1.0, 2.0)), mesh, material: mat, object: ObjectId(9), flags: 0 }).unwrap();
    let src = ShaderSources::builtin();
    let mut tex: Option<TextureHandle> = None;
    let mut reload = |rig: &mut Rig, i: u32| {
        let dev = &*rig.dev;
        let r = rig.renderer.as_mut().unwrap();
        // Texture: upload the new one, point the material at it, drop the old one.
        let mut m = Material::pbr(Vec3::new(1.0, 0.5 + 0.01 * i as f32, 0.5), 0.0, 0.6);
        if bc {
            let t = r.add_texture(dev, &baked(&solid(64, [(i * 5) as u8, 128, 200, 255]), Role::Colour)).unwrap();
            m = m.with_textures(MaterialTextures { base: Some(t), ..Default::default() });
            if let Some(old) = tex.replace(t) {
                r.remove_texture(dev, old);
            }
        }
        r.set_material(mat, m);
        // Mesh: alternate quad and cube in place.
        let asset = if i % 2 == 0 { cube_mesh() } else { quad() };
        r.replace_mesh(dev, mesh, &asset).unwrap();
        r.reload_shaders(dev, &src).unwrap();
    };
    for i in 0..5 {
        reload(&mut rig, i);
        for _ in 0..3 {
            rig.frame(cam, SceneMode::Scene, false);
        }
    }
    rig.settle();
    rig.frame(cam, SceneMode::Scene, false);
    rig.settle();
    let base = rig.dev.allocated_bytes();
    let geo = rig.r().geometry_in_use();
    for i in 5..55 {
        reload(&mut rig, i);
        for _ in 0..3 {
            rig.frame(cam, SceneMode::Scene, false);
        }
    }
    rig.settle();
    rig.frame(cam, SceneMode::Scene, false);
    rig.settle();
    let after = rig.dev.allocated_bytes();
    let geo_after = rig.r().geometry_in_use();
    println!("50 reloads: allocated {base} B -> {after} B; geometry (verts, indices, pending) {geo:?} -> {geo_after:?}; texture bytes {}", rig.r().texture_bytes());
    assert!(after <= base, "device memory grew from {base} to {after} bytes over 50 reloads");
    assert_eq!(geo_after.2, 0, "every deferred release collected");
    assert!(geo_after.0 <= geo.0.max(24 + 24) && geo_after.1 <= geo.1.max(36 + 36), "geometry ranges stay bounded: {geo:?} -> {geo_after:?}");
    rig.destroy();
}
