//! M4b: multi-view (P1-D24), the overlay pass, and in-place skinning with two frames in flight.
#![cfg(target_os = "macos")]

mod common;
use common::m3::*;
use common::*;
use ed_geo::ObjectId;
use ed_gpu::*;
use ed_render::scene::*;
use ed_render::*;
use glam::{Affine3A, DVec3, Mat4, Vec3};

fn decode(raw: &[u8]) -> Vec<[u8; 4]> {
    let pitch = OUT_FORMAT.copy_row_pitch(W) as usize;
    let mut px = Vec::with_capacity((W * H) as usize);
    for y in 0..H as usize {
        for x in 0..W as usize {
            let o = y * pitch + x * 4;
            px.push([raw[o], raw[o + 1], raw[o + 2], raw[o + 3]]);
        }
    }
    px
}

fn readback(dev: &dyn Device) -> BufferId {
    dev.create_buffer(&BufferDesc { label: "readback", size: (OUT_FORMAT.copy_row_pitch(W) * H) as u64, usage: Usage { copy_dst: true, cpu_read: true, ..Default::default() } }).unwrap()
}

/// Renders `views` in one frame (in order); the last view copies the output to `rb`.
fn frame(rig: &mut Rig, views: &[View], rb: Option<BufferId>, pick: Option<(u32, u32)>) {
    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    r.flush_uploads(dev).unwrap();
    let mut e = dev.begin_frame().unwrap();
    for (i, v) in views.iter().enumerate() {
        let last = i + 1 == views.len();
        let p = FrameParams { readback: if last { rb } else { None }, pick: if i == 0 { pick } else { None }, ..Default::default() };
        r.render(dev, &mut *e, v, &p).unwrap();
    }
    dev.end_frame(e).expect("end_frame (validation)");
}

fn inset(rig: &Rig, cam: Camera) -> View {
    let mut v = rig.view(cam);
    v.viewport = Viewport { x: W - 96, y: 0, width: 96, height: 96 };
    v.flags = ViewFlags { slot: 1, keep_output: true, overlay: false };
    v
}

/// The inset camera: above the scene, looking up (sees none of it), so its culling result
/// must differ from the main view's.
fn top_down() -> Camera {
    Camera { position: TestScene::ORIGIN + DVec3::new(0.0, 60.0, 0.0), yaw: 0.0, pitch: 1.5, fov_y: 1.0, near: 0.1 }
}

#[test]
fn second_view_is_an_inset_and_leaks_nothing_into_the_main_view() {
    let mut rig = Rig::new();
    let cam = TestScene::test_camera();
    let rb = rig.readback;
    rig.renderer.as_mut().unwrap().prepare_views(&*rig.dev, 2).unwrap();
    // Main view alone, settled over a few frames (LOD fades, occlusion history).
    for _ in 0..24 {
        let v = [rig.view(cam)];
        frame(&mut rig, &v, None, None);
    }
    let v = [rig.view(cam)];
    frame(&mut rig, &v, Some(rb), None);
    let alone = rig.read_image();
    rig.settle();
    let alone_stats = rig.r().cull_stats().unwrap();

    // Main view + inset, many frames.
    for _ in 0..24 {
        let v = [rig.view(cam), inset(&rig, top_down())];
        frame(&mut rig, &v, None, None);
    }
    let v = [rig.view(cam), inset(&rig, top_down())];
    frame(&mut rig, &v, Some(rb), None);
    let both = rig.read_image();
    rig.settle();
    assert_eq!(rig.r().view_slots(), 2);
    let main_stats = rig.r().cull_stats_for(0).unwrap();
    let inset_stats = rig.r().cull_stats_for(1).unwrap();
    println!("main alone {alone_stats:?}\nmain with inset {main_stats:?}\ninset {inset_stats:?}");
    // Per-view culling: the main view's counters are unchanged by the second view.
    assert_eq!((main_stats.drawn(), main_stats.after_frustum, main_stats.triangles), (alone_stats.drawn(), alone_stats.after_frustum, alone_stats.triangles));
    assert_ne!(inset_stats.after_frustum, main_stats.after_frustum, "the inset culls on its own frustum");

    // Outside the inset the image is the main view's, pixel for pixel.
    let mut outside_diff = 0;
    let mut inside_diff = 0;
    for y in 0..H {
        for x in 0..W {
            let d = (0..3).map(|c| (at(&alone, x, y)[c] as i32 - at(&both, x, y)[c] as i32).abs()).max().unwrap();
            if x >= W - 96 && y < 96 {
                inside_diff = inside_diff.max(d);
            } else {
                outside_diff = outside_diff.max(d);
            }
        }
    }
    println!("max diff outside inset {outside_diff}, inside {inside_diff}");
    assert_eq!(outside_diff, 0, "the inset must not touch the rest of the target (keep_output)");
    assert!(inside_diff > 20, "the inset draws its own view");

    // Picks are per view: a pick in the main view lands in slot 0 only.
    let v = [rig.view(cam), inset(&rig, top_down())];
    frame(&mut rig, &v, None, Some((W / 2, H / 2)));
    rig.settle();
    assert!(rig.r().pick_result_for(0).is_some());
    assert!(rig.r().pick_result_for(1).is_none());
    rig.destroy();
}

#[test]
fn overlay_draws_lines_and_triangles_on_top_without_depth() {
    let mut rig = Rig::new();
    let cam = TestScene::test_camera();
    let base = rig.image(cam);
    // A magenta triangle and a green line a few metres in front of the camera, relative to
    // an origin far from zero (f64 origin, f32 offsets).
    let origin = cam.position + cam.forward().as_dvec3() * 4.0;
    let mag = [255, 0, 255, 255];
    let tri = [
        OverlayVertex { pos: [-0.5, -0.5, 0.0], color: mag },
        OverlayVertex { pos: [0.5, -0.5, 0.0], color: mag },
        OverlayVertex { pos: [0.0, 0.5, 0.0], color: mag },
    ];
    let r = cam.right();
    let green = [0, 255, 0, 255];
    let line = [
        OverlayVertex { pos: (r * -1.5 + Vec3::Y * 0.9).to_array(), color: green },
        OverlayVertex { pos: (r * 1.5 + Vec3::Y * 0.9).to_array(), color: green },
    ];
    rig.r().set_overlay(origin, &line, &tri);
    assert!(rig.r().pass_names(&FrameParams::default()).contains(&"overlay"));
    let img = rig.image(cam);
    let c = at(&img, W / 2, H / 2 + 8);
    println!("centre with overlay {c:?}, without {:?}", at(&base, W / 2, H / 2 + 8));
    assert!(c[0] > 200 && c[2] > 200 && c[1] < 40, "magenta triangle at the centre: {c:?}");
    let greens = img.iter().filter(|p| p[1] > 200 && p[0] < 60 && p[2] < 60).count();
    assert!(greens >= W as usize / 3, "a green line across the view ({greens} px)");
    // Cleared: the frame is the plain scene again.
    rig.r().set_overlay(DVec3::ZERO, &[], &[]);
    let after = rig.image(cam);
    assert_eq!(hash(&after), hash(&base));
    rig.destroy();
}

fn bend(a: f32) -> [Mat4; 2] {
    let pivot = Vec3::new(0.0, 1.0, 0.0);
    [Mat4::IDENTITY, Mat4::from_translation(pivot) * Mat4::from_rotation_z(a) * Mat4::from_translation(-pivot)]
}

/// Skinning writes the posed vertices in place into the shared vertex buffer. With two frames
/// in flight, frame N+1's skin pass must not overwrite vertices frame N's raster/resolve still
/// read. Each frame alternates the pose and copies its output; every frame must match the
/// single-frame reference for its own pose exactly. See `docs` in the M4b report for why this
/// holds (the vertex buffer's final state forces a barrier at the start of every frame).
#[test]
fn in_place_skinning_is_safe_with_two_frames_in_flight() {
    let mut rig = Rig::bare();
    let dev = &*rig.dev;
    let bar = skinned_bar();
    let r = rig.renderer.as_mut().unwrap();
    let red = r.add_material(Material::pbr(Vec3::new(0.8, 0.05, 0.05), 0.0, 0.6)).unwrap();
    let (skin, mesh) = r.add_skinned_mesh(dev, &bar).unwrap();
    let node = r.instances().add_node(TestScene::ORIGIN).unwrap();
    r.instances().add(&InstanceDesc { node, transform: Affine3A::IDENTITY, mesh, material: red, object: ObjectId(1), flags: 0 }).unwrap();
    let cam = Camera::look_at(TestScene::ORIGIN + DVec3::new(0.0, 1.0, 5.0), TestScene::ORIGIN + DVec3::new(0.0, 1.0, 0.0));
    let poses = [bend(1.4), bend(-1.0)];
    // References: one frame at a time, idle in between. LOD cross-fades settle first.
    let mut refs = Vec::new();
    for p in &poses {
        rig.r().set_skin_pose(skin, p).unwrap();
        for _ in 0..20 {
            rig.frame(cam, SceneMode::Scene, false);
        }
        refs.push(rig.image(cam));
    }
    assert_ne!(hash(&refs[0]), hash(&refs[1]));

    // Pipelined: never wait; four readback buffers, one per frame mod 4.
    let rbs: Vec<BufferId> = (0..4).map(|_| readback(&*rig.dev)).collect();
    let mut pending: Vec<(u64, usize, usize)> = Vec::new(); // (frame, buffer, pose)
    let (mut checked, mut overlapped) = (0, 0);
    let mut raw = Vec::new();
    for i in 0..40usize {
        let pose = i % 2;
        rig.r().set_skin_pose(skin, &poses[pose]).unwrap();
        let dev = &*rig.dev;
        let r = rig.renderer.as_mut().unwrap();
        r.flush_uploads(dev).unwrap();
        let mut e = dev.begin_frame().unwrap();
        let fi = e.frame_index();
        // Read back every finished frame (a buffer is reused 4 frames later; with two frames
        // in flight, begin_frame(N) has waited for N-2, so N-4 is done).
        let done = dev.completed_frames();
        pending.retain(|&(f, b, p)| {
            if f < done {
                dev.read_buffer(rbs[b], &mut raw).unwrap();
                let img = decode(&raw);
                assert_eq!(hash(&img), hash(&refs[p]), "frame {f} (pose {p}) differs from its single-frame reference");
                checked += 1;
                false
            } else {
                true
            }
        });
        assert!(pending.iter().all(|&(_, b, _)| b != i % 4), "readback buffer still pending");
        r.render(dev, &mut *e, &rig_view(&rig.target, cam), &FrameParams { readback: Some(rbs[i % 4]), ..Default::default() }).unwrap();
        dev.end_frame(e).expect("end_frame (validation)");
        if dev.completed_frames() <= fi.saturating_sub(1) && fi > 0 {
            overlapped += 1; // the previous frame was still running when this one was submitted
        }
        pending.push((fi, i % 4, pose));
    }
    rig.dev.wait_idle();
    for &(f, b, p) in &pending {
        rig.dev.read_buffer(rbs[b], &mut raw).unwrap();
        assert_eq!(hash(&decode(&raw)), hash(&refs[p]), "frame {f} (pose {p})");
        checked += 1;
    }
    println!("checked {checked} pipelined frames, {overlapped} submitted while the previous frame was in flight");
    assert_eq!(checked, 40);
    for b in rbs {
        rig.dev.destroy_buffer(b);
    }
    rig.destroy();
}

fn rig_view(target: &TextureId, camera: Camera) -> View {
    View {
        camera,
        viewport: Viewport { x: 0, y: 0, width: W, height: H },
        target: ViewTarget { texture: *target, format: OUT_FORMAT, width: W, height: H, initial: ResourceState::Undefined, final_state: None },
        flags: ViewFlags::default(),
    }
}
