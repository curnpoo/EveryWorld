//! M2 GPU tests: culling counts, LOD selection, cross-fade length, PBR and toon under one
//! light, picking. Small hand-made scenes at 256x256 (Metal).
#![cfg(target_os = "macos")]

mod common;
use common::*;
use ed_geo::ObjectId;
use ed_render::provider::{pixels_per_metre, select_lod};
use ed_render::scene::*;
use ed_render::*;
use glam::{Affine3A, DVec3, Quat, Vec3};

const ORIGIN: DVec3 = DVec3::new(-2_100_000.0, 4_300_000.0, 1_800_000.0);

fn add(r: &mut Renderer, node: NodeSlot, mesh: MeshId, mat: MaterialId, p: Vec3, s: Vec3, object: u128) -> InstanceId {
    r.instances()
        .add(&InstanceDesc {
            node,
            transform: Affine3A::from_scale_rotation_translation(s, Quat::IDENTITY, p),
            mesh,
            material: mat,
            object: ObjectId(object),
            flags: 0,
        })
        .unwrap()
}

/// Camera at the node origin + `eye`, looking down -Z.
fn camera(eye: Vec3) -> Camera {
    Camera { position: ORIGIN + eye.as_dvec3(), yaw: 0.0, pitch: 0.0, fov_y: Camera::DEFAULT_FOV_Y, near: 0.1 }
}

/// Pixel of a node-relative point for `cam` (W x H target).
fn pixel(cam: &Camera, p: Vec3) -> (u32, u32) {
    let rel = (ORIGIN + p.as_dvec3() - cam.position).as_vec3();
    let c = cam.relative_view_proj(W as f32 / H as f32) * rel.extend(1.0);
    let (x, y) = (c.x / c.w, c.y / c.w);
    (((x * 0.5 + 0.5) * W as f32) as u32, ((0.5 - y * 0.5) * H as f32) as u32)
}

fn pick(rig: &mut Rig, cam: Camera, at: (u32, u32)) -> PickResult {
    rig.frame_with(cam, &FrameParams { pick: Some(at), ..Default::default() });
    rig.settle();
    let p = rig.r().pick_result().expect("pick answer");
    assert_eq!((p.x, p.y), at);
    p
}

#[test]
fn culling_counts_frustum_and_occlusion() {
    let Some(mut rig) = Rig::bare() else { return };
    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    let cube = r.add_mesh(dev, &cube_mesh()).unwrap();
    let mat = r.add_material(Material::pbr(Vec3::new(0.6, 0.6, 0.6), 0.0, 0.5)).unwrap();
    let node = r.instances().add_node(ORIGIN).unwrap();
    let one = Vec3::ONE;
    add(r, node, cube, mat, Vec3::new(-3.0, 0.0, -20.0), one * 2.0, 1); // visible
    add(r, node, cube, mat, Vec3::new(0.0, 0.0, 25.0), one * 2.0, 2); // behind the camera
    add(r, node, cube, mat, Vec3::new(200.0, 0.0, -20.0), one * 2.0, 3); // outside the frustum
    add(r, node, cube, mat, Vec3::new(4.0, 0.0, -30.0), Vec3::new(14.0, 14.0, 1.0), 4); // occluder wall
    add(r, node, cube, mat, Vec3::new(4.0, 0.0, -60.0), one * 2.0, 5); // behind the wall
    let cam = camera(Vec3::ZERO);
    for _ in 0..4 {
        rig.frame(cam, SceneMode::Scene, false);
    }
    rig.settle();
    let s = rig.r().cull_stats().expect("stats");
    println!("{s:?}");
    assert_eq!(s.total, 5);
    assert_eq!(s.after_frustum, 3, "behind and outside are frustum-culled");
    assert_eq!(s.after_occlusion, 2, "the cube behind the wall is occluded");
    assert_eq!(s.drawn(), 2);
    assert_eq!(s.drawn_early, 2, "steady state: everything drawn in the early phase");
    assert_eq!(s.drawn_late, 0);
    // The visible cube is under its pixel; the wall hides the far one.
    let p = pick(&mut rig, cam, pixel(&cam, Vec3::new(-3.0, 0.0, -20.0)));
    assert_eq!(p.object, Some(ObjectId(1)));
    let p = pick(&mut rig, cam, pixel(&cam, Vec3::new(4.0, 0.0, -60.0)));
    assert_eq!(p.object, Some(ObjectId(4)), "the wall, not the cube behind it");
    // Without the occlusion test the hidden cube counts as visible (and is drawn, under the wall).
    rig.r().lod_config().occlusion = false;
    for _ in 0..3 {
        rig.frame(cam, SceneMode::Scene, false);
    }
    rig.settle();
    let s = rig.r().cull_stats().unwrap();
    assert_eq!((s.after_frustum, s.after_occlusion, s.drawn()), (3, 3, 3), "{s:?}");
    rig.destroy();
}

#[test]
fn camera_move_reveals_occluded_instance_same_frame() {
    // Two-phase occlusion: an instance hidden last frame but visible now is drawn in the late
    // phase of this frame (no one-frame hole).
    let Some(mut rig) = Rig::bare() else { return };
    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    let cube = r.add_mesh(dev, &cube_mesh()).unwrap();
    let mat = r.add_material(Material::pbr(Vec3::new(0.6, 0.2, 0.2), 0.0, 0.5)).unwrap();
    let node = r.instances().add_node(ORIGIN).unwrap();
    add(r, node, cube, mat, Vec3::new(0.0, 0.0, -30.0), Vec3::new(10.0, 10.0, 1.0), 10);
    add(r, node, cube, mat, Vec3::new(0.0, 0.0, -60.0), Vec3::ONE * 2.0, 11);
    let cam = camera(Vec3::ZERO);
    for _ in 0..3 {
        rig.frame(cam, SceneMode::Scene, false);
    }
    let moved = camera(Vec3::new(40.0, 0.0, 0.0));
    let moved = Camera { yaw: (40.0f32).atan2(60.0), ..moved };
    let at = pixel(&moved, Vec3::new(0.0, 0.0, -60.0));
    let p = pick(&mut rig, moved, at);
    assert_eq!(p.object, Some(ObjectId(11)), "revealed cube drawn in the frame it appears");
    let s = rig.r().cull_stats().unwrap();
    assert!(s.drawn_late >= 1, "{s:?}");
    rig.destroy();
}

/// A cube with four "LODs" (same triangles) and known errors.
fn lod_cube() -> MeshAsset {
    let mut m = cube_mesh();
    let idx = m.lods[0].indices.clone();
    m.lods = [0.0, 0.01, 0.05, 0.2].iter().map(|&e| MeshLod { indices: idx.clone(), error: e }).collect();
    m
}

#[test]
fn lod_level_at_known_distances() {
    let Some(mut rig) = Rig::bare() else { return };
    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    r.lod_config().fade_frames = 0;
    let mesh = r.add_mesh(dev, &lod_cube()).unwrap();
    let mat = r.add_material(Material::pbr(Vec3::new(0.5, 0.5, 0.5), 0.0, 0.5)).unwrap();
    let node = r.instances().add_node(ORIGIN).unwrap();
    let scale = 4.0;
    add(r, node, mesh, mat, Vec3::ZERO, Vec3::ONE * scale, 7);
    let errors = [0.0, 0.01, 0.05, 0.2];
    let radius = 0.5 * 3f32.sqrt() * scale;
    let p11 = 1.0 / (0.5 * Camera::DEFAULT_FOV_Y).tan();
    // Far to near, so every step goes finer (no hysteresis involved).
    let mut got = Vec::new();
    for (d, want) in [(400.0f32, 3u32), (120.0, 2), (30.0, 1), (6.0, 0)] {
        let cam = camera(Vec3::new(0.0, 0.0, d));
        for _ in 0..2 {
            rig.frame(cam, SceneMode::Scene, false);
        }
        let p = pick(&mut rig, cam, (W / 2, H / 2));
        let k = pixels_per_metre(scale, H as f32, p11, d, radius, 0.1);
        assert_eq!(select_lod(&errors, k, 3, false, 1.0, 0.75), want, "CPU reference at {d} m");
        assert_eq!(p.object, Some(ObjectId(7)));
        got.push((d, p.lod));
        assert_eq!(p.lod, want, "LOD at {d} m (k = {k} px/m)");
    }
    println!("lod by distance: {got:?}");
    // Hysteresis: back out to 30 m from 6 m. Level 1 projects 0.37 px there (< 0.75): switches.
    // At 22 m it projects 0.51 px (< 0.75) too; at 18 m 0.66 px. At 14 m 0.86 px: allowed by
    // the 1 px threshold but not by the hysteresis band, so it stays at 0.
    let cam = camera(Vec3::new(0.0, 0.0, 14.0));
    for _ in 0..2 {
        rig.frame(cam, SceneMode::Scene, false);
    }
    assert_eq!(pick(&mut rig, cam, (W / 2, H / 2)).lod, 0, "hysteresis keeps the finer level");
    rig.destroy();
}

#[test]
fn cross_fade_draws_both_levels_for_exactly_the_fade_length() {
    const N: u32 = 8;
    let Some(mut rig) = Rig::bare() else { return };
    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    r.lod_config().fade_frames = N;
    let mesh = r.add_mesh(dev, &lod_cube()).unwrap();
    let mat = r.add_material(Material::pbr(Vec3::new(0.5, 0.5, 0.5), 0.0, 0.5)).unwrap();
    let node = r.instances().add_node(ORIGIN).unwrap();
    add(r, node, mesh, mat, Vec3::ZERO, Vec3::ONE * 4.0, 9);
    let far = camera(Vec3::new(0.0, 0.0, 400.0));
    for _ in 0..4 {
        rig.frame(far, SceneMode::Scene, false);
    }
    rig.settle();
    assert_eq!(rig.r().cull_stats().unwrap().entries, 1);
    let near = camera(Vec3::new(0.0, 0.0, 6.0));
    let mut entries = Vec::new();
    for _ in 0..N + 4 {
        rig.frame(near, SceneMode::Scene, false);
        rig.settle();
        let s = rig.r().cull_stats().unwrap();
        assert_eq!(s.drawn(), 1);
        entries.push(s.entries);
    }
    println!("entries per frame after the switch: {entries:?}");
    let two = entries.iter().filter(|&&e| e == 2).count() as u32;
    assert_eq!(two, N, "both levels drawn for exactly {N} frames");
    assert!(entries[..N as usize].iter().all(|&e| e == 2), "the fade starts at the switch");
    assert!(entries[N as usize..].iter().all(|&e| e == 1), "then one level");
    // fade 0: instant switch, never two entries.
    rig.r().lod_config().fade_frames = 0;
    for _ in 0..3 {
        rig.frame(far, SceneMode::Scene, false);
        rig.settle();
        assert_eq!(rig.r().cull_stats().unwrap().entries, 1);
    }
    rig.destroy();
}

fn luma(p: [u8; 4]) -> f32 {
    0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32
}

#[test]
fn pbr_and_toon_render_under_the_same_light() {
    let Some(mut rig) = Rig::bare() else { return };
    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    let cube = r.add_mesh(dev, &cube_mesh()).unwrap();
    let red = Vec3::new(0.7, 0.06, 0.05);
    let pbr = r.add_material(Material::pbr(red, 0.0, 0.6)).unwrap();
    let toon = r.add_material(Material::toon(red, Vec3::splat(0.4), 3.0, 0.5)).unwrap();
    let node = r.instances().add_node(ORIGIN).unwrap();
    add(r, node, cube, pbr, Vec3::new(-1.5, 0.0, 0.0), Vec3::ONE * 2.0, 21);
    add(r, node, cube, toon, Vec3::new(1.5, 0.0, 0.0), Vec3::ONE * 2.0, 22);
    // Above and in front, looking down at both: top faces and front faces visible.
    let eye = Vec3::new(0.0, 4.0, 8.0);
    let cam = Camera { position: ORIGIN + eye.as_dvec3(), ..Camera::look_at(ORIGIN + eye.as_dvec3(), ORIGIN) };
    rig.frame(cam, SceneMode::Scene, true);
    let px = rig.read_image();
    let sample = |p: Vec3| {
        let (x, y) = pixel(&cam, p);
        at(&px, x, y)
    };
    let (pbr_top, pbr_front) = (sample(Vec3::new(-1.5, 1.0, 0.0)), sample(Vec3::new(-1.5, 0.0, 1.0)));
    let (toon_top, toon_front) = (sample(Vec3::new(1.5, 1.0, 0.0)), sample(Vec3::new(1.5, 0.0, 1.0)));
    println!("pbr top {pbr_top:?} front {pbr_front:?}; toon top {toon_top:?} front {toon_front:?}");
    for (name, p) in [("pbr top", pbr_top), ("pbr front", pbr_front), ("toon top", toon_top), ("toon front", toon_front)] {
        assert!(p[0] as i32 > p[1] as i32 + 40 && p[0] as i32 > p[2] as i32 + 40, "{name} is red: {p:?}");
    }
    // Sun (0.45, 0.62, 0.35): the top faces get more light than the front faces, in both models.
    assert!(luma(pbr_top) > luma(pbr_front) + 5.0, "pbr: top lit more than front");
    assert!(luma(toon_top) > luma(toon_front) + 5.0, "toon: top lit more than front");
    assert_ne!(pbr_top, toon_top, "the two models shade differently");
    rig.destroy();
}

#[test]
fn picking_returns_the_object_and_sky() {
    let Some(mut rig) = Rig::new() else { return };
    let cam = TestScene::test_camera();
    let p = pick(&mut rig, cam, (W / 2, H / 2));
    assert_eq!(p.object, Some(ObjectId(0x7E57_0001)), "the red box");
    let p = pick(&mut rig, cam, (W / 2, 2));
    assert_eq!((p.instance, p.object), (None, None), "sky");
    let p = pick(&mut rig, cam, (W / 2, H - 2));
    assert_eq!(p.object, Some(ObjectId(0x7E57_0000)), "the ground");
    rig.destroy();
}
