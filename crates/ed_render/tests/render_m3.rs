//! M3 renderer features: bindless material textures (BC7 / BC5 baked by the asset pipeline),
//! GPU skinning, in-place mesh replacement with deferred frees, shader hot reload.
#![cfg(target_os = "macos")]

mod common;
use common::m3::*;
use common::*;
use ed_geo::ObjectId;
use ed_render::scene::*;
use ed_render::shaders::ShaderSources;
use ed_render::*;
use glam::{Affine3A, Mat4, Quat, Vec3};

fn cam_at(from: Vec3, to: Vec3) -> Camera {
    Camera::look_at(TestScene::ORIGIN + from.as_dvec3(), TestScene::ORIGIN + to.as_dvec3())
}

fn place(r: &mut Renderer, mesh: MeshId, mat: MaterialId, t: Affine3A) -> InstanceId {
    let node = match r.instance_pools().nodes().first() {
        Some(_) => NodeSlot(0),
        None => r.instances().add_node(TestScene::ORIGIN).unwrap(),
    };
    r.instances().add(&InstanceDesc { node, transform: t, mesh, material: mat, object: ObjectId(1), flags: 0 }).unwrap()
}

fn bc(rig: &Rig) -> bool {
    let ok = rig.dev.caps().bc_textures;
    if !ok {
        eprintln!("no BC texture support; skipping");
    }
    ok
}

fn max_diff(a: &[[u8; 4]], b: &[[u8; 4]]) -> i32 {
    a.iter().zip(b).flat_map(|(x, y)| (0..3).map(move |c| (x[c] as i32 - y[c] as i32).abs())).max().unwrap_or(0)
}

#[test]
fn base_colour_normal_and_orm_textures_are_sampled() {
    let Some(mut rig) = Rig::bare() else { return };
    if !bc(&rig) {
        return rig.destroy();
    }
    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    let mesh = r.add_mesh(dev, &quad()).unwrap();
    let plain = r.add_material(Material::pbr(Vec3::ONE, 0.0, 0.8)).unwrap();
    let inst = place(r, mesh, plain, Affine3A::from_scale_rotation_translation(Vec3::splat(4.0), Quat::IDENTITY, Vec3::new(0.0, 1.0, 0.0)));
    let cam = cam_at(Vec3::new(0.0, 1.0, 3.0), Vec3::new(0.0, 1.0, 0.0));
    let untextured = rig.image(cam);

    // Base colour: red / green / blue / white quadrants.
    let q = quadrants(64, [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 255, 255]]);
    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    let base = r.add_texture(dev, &baked(&q, ed_assets::texture::Role::Colour)).unwrap();
    let textured = r.add_material(Material::pbr(Vec3::ONE, 0.0, 0.8).with_textures(MaterialTextures { base: Some(base), ..Default::default() })).unwrap();
    r.instances().set_material(inst, textured);
    let img = rig.image(cam);
    let (lo, hi) = (W / 2 - 40, W / 2 + 40);
    let tl = at(&img, lo, lo);
    let tr = at(&img, hi, lo);
    let bl = at(&img, lo, hi);
    let br = at(&img, hi, hi);
    println!("quadrants: tl {tl:?} tr {tr:?} bl {bl:?} br {br:?}");
    assert!(tl[0] > tl[1] + 60 && tl[0] > tl[2] + 60, "top-left red: {tl:?}");
    assert!(tr[1] > tr[0] + 60 && tr[1] > tr[2] + 60, "top-right green: {tr:?}");
    assert!(bl[2] > bl[0] + 60 && bl[2] > bl[1] + 60, "bottom-left blue: {bl:?}");
    assert!(br.iter().take(3).all(|c| *c > 100), "bottom-right white: {br:?}");
    assert!(max_diff(&img, &untextured) > 100);

    // Normal map: a flat one changes nothing (within BC5 rounding); a tilted one does.
    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    let flat = r.add_texture(dev, &baked(&solid(32, [128, 128, 255, 255]), ed_assets::texture::Role::Normal)).unwrap();
    let tilt = r.add_texture(dev, &baked(&solid(32, [128, 230, 200, 255]), ed_assets::texture::Role::Normal)).unwrap();
    let orm = r.add_texture(dev, &baked(&solid(32, [255, 30, 0, 255]), ed_assets::texture::Role::Linear)).unwrap();
    let m_flat = r.add_material(Material::pbr(Vec3::splat(0.25), 0.0, 0.8).with_textures(MaterialTextures { normal: Some(flat), ..Default::default() })).unwrap();
    let m_tilt = r.add_material(Material::pbr(Vec3::splat(0.25), 0.0, 0.8).with_textures(MaterialTextures { normal: Some(tilt), ..Default::default() })).unwrap();
    let m_orm = r.add_material(Material::pbr(Vec3::splat(0.25), 0.0, 0.8).with_textures(MaterialTextures { orm: Some(orm), ..Default::default() })).unwrap();
    let m_grey = r.add_material(Material::pbr(Vec3::splat(0.25), 0.0, 0.8)).unwrap();
    r.instances().set_material(inst, m_grey);
    let untextured = rig.image(cam);
    rig.r().instances().set_material(inst, m_flat);
    let img_flat = rig.image(cam);
    rig.r().instances().set_material(inst, m_tilt);
    let img_tilt = rig.image(cam);
    rig.r().instances().set_material(inst, m_orm);
    let img_orm = rig.image(cam);
    let (d_flat, d_tilt, d_orm) = (max_diff(&img_flat, &untextured), max_diff(&img_tilt, &untextured), max_diff(&img_orm, &untextured));
    println!("centre px: grey {:?} flat {:?} tilt {:?} orm {:?}", at(&untextured, W / 2, H / 2), at(&img_flat, W / 2, H / 2), at(&img_tilt, W / 2, H / 2), at(&img_orm, W / 2, H / 2));
    println!("normal map flat diff {d_flat}, tilted diff {d_tilt}, orm (rough x0.12) diff {d_orm}");
    assert!(d_flat <= 3, "flat normal map must match the geometric normal ({d_flat})");
    assert!(d_tilt > 10, "tilted normal map must change shading ({d_tilt})");
    // Green up = +V up the image = world +Y here: the surface turns toward the sky and sun.
    let luma = |img: &[[u8; 4]]| img.iter().map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64).sum::<u64>();
    assert!(luma(&img_tilt) > luma(&img_flat), "up-tilted normals must brighten (bitangent sign)");
    assert!(d_orm > 0, "ORM roughness 0.1 must change shading ({d_orm})");
    rig.destroy();
}

/// Joint matrix bending the bar's upper half by `a` radians about Z at y = 1.
fn bend(a: f32) -> [Mat4; 2] {
    let pivot = Vec3::new(0.0, 1.0, 0.0);
    [Mat4::IDENTITY, Mat4::from_translation(pivot) * Mat4::from_rotation_z(a) * Mat4::from_translation(-pivot)]
}

fn red_pixels(img: &[[u8; 4]], x0: u32, x1: u32, y0: u32, y1: u32) -> usize {
    (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).filter(|&(x, y)| {
        let p = at(img, x, y);
        p[0] as i32 > p[2] as i32 + 40 && p[0] as i32 > p[1] as i32 + 40
    }).count()
}

#[test]
fn skinning_pass_moves_vertices_and_identity_matches_the_static_mesh() {
    let Some(mut rig) = Rig::bare() else { return };
    let dev = &*rig.dev;
    let bar = skinned_bar();
    let r = rig.renderer.as_mut().unwrap();
    let red = r.add_material(Material::pbr(Vec3::new(0.8, 0.05, 0.05), 0.0, 0.6)).unwrap();
    let static_mesh = r.add_mesh(dev, &bar.mesh).unwrap();
    let s = place(r, static_mesh, red, Affine3A::IDENTITY);
    let cam = cam_at(Vec3::new(0.0, 1.0, 5.0), Vec3::new(0.0, 1.0, 0.0));
    let static_img = rig.image(cam);
    assert!(!rig.r().pass_names(&FrameParams::default()).contains(&"skin"));

    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    r.instances().remove(s);
    let (skin, mesh) = r.add_skinned_mesh(dev, &bar).unwrap();
    place(r, mesh, red, Affine3A::IDENTITY);
    assert_eq!(r.pass_names(&FrameParams::default())[0], "skin");
    let ident = rig.image(cam);
    let d = max_diff(&ident, &static_img);
    println!("identity skinning vs static mesh: max channel diff {d}");
    assert!(d <= 2, "identity pose must render like the bind-pose mesh ({d})");

    // Bend the top half to -X: red appears left of the bar's upper part.
    let left = |img: &[[u8; 4]]| red_pixels(img, 0, W / 2 - 30, 0, H / 2);
    let before = left(&ident);
    rig.r().set_skin_pose(skin, &bend(std::f32::consts::FRAC_PI_2)).unwrap();
    let bent = rig.image(cam);
    let after = left(&bent);
    println!("red pixels upper-left: identity {before}, bent {after}");
    assert!(after > before + 80, "bent pose must move the upper half left");
    // Two poses differ from each other and are repeatable.
    rig.r().set_skin_pose(skin, &bend(-0.6)).unwrap();
    let other = rig.image(cam);
    rig.r().set_skin_pose(skin, &bend(std::f32::consts::FRAC_PI_2)).unwrap();
    let again = rig.image(cam);
    assert_ne!(hash(&other), hash(&bent));
    assert_eq!(hash(&again), hash(&bent), "same pose, same pixels");
    rig.destroy();
}

#[test]
fn replace_mesh_swaps_geometry_and_frees_old_ranges_after_frames() {
    let Some(mut rig) = Rig::bare() else { return };
    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    let mesh = r.add_mesh(dev, &cube_mesh()).unwrap();
    let m = r.add_material(Material::pbr(Vec3::new(0.7, 0.1, 0.1), 0.0, 0.7)).unwrap();
    place(r, mesh, m, Affine3A::from_translation(Vec3::new(0.0, 1.0, 0.0)));
    let cam = cam_at(Vec3::new(0.0, 1.5, 4.0), Vec3::new(0.0, 1.0, 0.0));
    let cube = rig.image(cam);
    let used_cube = rig.r().geometry_in_use();
    let dev = &*rig.dev;
    rig.renderer.as_mut().unwrap().replace_mesh(dev, mesh, &quad()).unwrap();
    let (v, i, pending) = rig.r().geometry_in_use();
    assert_eq!(pending, 2, "old vertex and index ranges wait for the frames in flight");
    assert_eq!((v, i), (used_cube.0 + 4, used_cube.1 + 6));
    let q = rig.image(cam);
    assert_ne!(hash(&cube), hash(&q));
    rig.settle();
    rig.frame(cam, SceneMode::Scene, false); // flush collects completed releases
    assert_eq!(rig.r().geometry_in_use(), (4, 6, 0));
    // Remove + re-add reuses the id after its frames finished.
    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    let extra = r.add_mesh(dev, &cube_mesh()).unwrap();
    r.remove_mesh(extra).unwrap();
    rig.settle();
    rig.frame(cam, SceneMode::Scene, false);
    let dev = &*rig.dev;
    let again = rig.renderer.as_mut().unwrap().add_mesh(dev, &cube_mesh()).unwrap();
    assert_eq!(again, extra);
    rig.destroy();
}

#[test]
fn shader_reload_swaps_on_success_and_keeps_running_on_error() {
    let Some(mut rig) = Rig::new() else { return };
    let cam = TestScene::test_camera();
    let before = rig.image(cam);
    let dev = &*rig.dev;
    let mut src = ShaderSources::builtin();
    rig.renderer.as_mut().unwrap().reload_shaders(dev, &src).unwrap();
    assert_eq!(hash(&rig.image(cam)), hash(&before));
    // A visible edit: tonemap output scaled.
    src.resolve = src.resolve.replace("return float4(col, 1.0);", "return float4(col * float3(0.2, 1.0, 0.2), 1.0);");
    let dev = &*rig.dev;
    rig.renderer.as_mut().unwrap().reload_shaders(dev, &src).unwrap();
    let edited = rig.image(cam);
    assert_ne!(hash(&edited), hash(&before));
    // A broken edit: error, old pipelines keep drawing.
    src.resolve.push_str("\nthis is not msl;\n");
    let dev = &*rig.dev;
    let err = rig.renderer.as_mut().unwrap().reload_shaders(dev, &src).err().expect("broken shader must fail");
    println!("broken shader error: {}", format!("{err}").lines().next().unwrap_or(""));
    assert_eq!(hash(&rig.image(cam)), hash(&edited));
    // The source directory loads and matches the built-in copy.
    assert_eq!(ShaderSources::load(ShaderSources::source_dir()).unwrap(), ShaderSources::builtin());
    rig.destroy();
}

