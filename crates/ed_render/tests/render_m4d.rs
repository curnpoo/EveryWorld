//! M4d: baseline lighting (cascaded sun shadows, sky ambient, GTAO) and the alpha's tone map.
#![cfg(target_os = "macos")]

mod common;
use common::*;
use ed_geo::ObjectId;
use ed_gpu::*;
use ed_render::scene::*;
use ed_render::tonemap::{ALPHA_EXPOSURE, alpha_tonemap, test_ramp};
use ed_render::*;

/// Renders the tone-ramp frame into a fresh `format` target and returns RGBA8 pixels.
fn tone_ramp(dev: &dyn Device, r: &mut Renderer, format: Format) -> Vec<[u8; 4]> {
    let target = dev
        .create_texture(&TextureDesc {
            label: "tone ramp",
            kind: TextureKind::D2,
            format,
            width: W,
            height: H,
            depth_or_layers: 1,
            mips: 1,
            samples: 1,
            usage: Usage { render_target: true, copy_src: true, ..Default::default() },
        })
        .unwrap();
    let pitch = format.copy_row_pitch(W) as usize;
    let readback = dev
        .create_buffer(&BufferDesc { label: "tone readback", size: (pitch * H as usize) as u64, usage: Usage { copy_dst: true, cpu_read: true, ..Default::default() } })
        .unwrap();
    let view = View {
        camera: TestScene::test_camera(),
        viewport: Viewport { x: 0, y: 0, width: W, height: H },
        target: ViewTarget { texture: target, format, width: W, height: H, initial: ResourceState::Undefined, final_state: None },
        flags: Default::default(),
    };
    let params = FrameParams { mode: SceneMode::ToneRamp, exposure: ALPHA_EXPOSURE, readback: Some(readback), ..Default::default() };
    r.flush_uploads(dev).unwrap();
    let mut e = dev.begin_frame().unwrap();
    r.render(dev, &mut *e, &view, &params).unwrap();
    dev.end_frame(e).expect("end_frame (validation)");
    dev.wait_idle();
    let mut raw = Vec::new();
    dev.read_buffer(readback, &mut raw).unwrap();
    let px = (0..H as usize).flat_map(|y| (0..W as usize).map(move |x| (y, x))).map(|(y, x)| {
        let o = y * pitch + x * 4;
        [raw[o], raw[o + 1], raw[o + 2], raw[o + 3]]
    });
    let px: Vec<[u8; 4]> = px.collect();
    dev.destroy_texture(target);
    dev.destroy_buffer(readback);
    px
}

/// GPU tone map vs the CPU transcription over a 16-stop ramp per channel plus saturated
/// colours, on a UNORM target (display value written as is) and an sRGB target (decoded, then
/// encoded by the hardware): every byte within 1/255.
#[test]
fn tonemap_matches_cpu_within_one_255th() {
    let Some(mut rig) = Rig::bare() else { return };
    for format in [Format::Rgba8Unorm, Format::Rgba8Srgb] {
        let dev = &*rig.dev;
        let px = tone_ramp(dev, rig.renderer.as_mut().unwrap(), format);
        let mut max = [0i32; 3];
        let mut worst = (0, 0);
        for y in 0..H {
            for x in 0..W {
                let want = alpha_tonemap(test_ramp(x, y, W, H), ALPHA_EXPOSURE);
                let got = px[(y * W + x) as usize];
                for k in 0..3 {
                    let e = (got[k] as i32 - (want[k] * 255.0).round() as i32).abs();
                    if e > max[k] {
                        max[k] = e;
                        worst = (x, y);
                    }
                }
            }
        }
        println!("tonemap {format:?}: max error per channel (R, G, B) = {max:?} /255 (last worst at {worst:?})");
        assert!(max.iter().all(|&e| e <= 1), "{format:?}: GPU tone map off by {max:?}/255 at {worst:?}");
    }
    rig.destroy();
}

// ---- Scenes for the lighting tests -----------------------------------------------------------

/// Unit-cube boxes (centre, size, colour) on one node at `TestScene::ORIGIN`.
fn load_boxes(rig: &mut Rig, boxes: &[([f32; 3], [f32; 3], [f32; 3])]) {
    let dev = &*rig.dev;
    let r = rig.renderer.as_mut().unwrap();
    let mesh = r.add_mesh(dev, &cube_mesh()).unwrap();
    let node = r.instances().add_node(TestScene::ORIGIN).unwrap();
    for (i, (p, s, c)) in boxes.iter().enumerate() {
        let mat = r.add_material(Material::pbr(glam::Vec3::from(*c), 0.0, 0.8)).unwrap();
        let t = glam::Affine3A::from_scale_rotation_translation(glam::Vec3::from(*s), glam::Quat::IDENTITY, glam::Vec3::from(*p));
        r.instances().add(&InstanceDesc { node, transform: t, mesh, material: mat, object: ObjectId(0x4D_0000 + i as u128), flags: 0 }).unwrap();
    }
}

const GROUND: ([f32; 3], [f32; 3], [f32; 3]) = ([0.0, -0.25, 0.0], [3000.0, 0.5, 3000.0], [0.45, 0.45, 0.42]);

fn sun() -> glam::Vec3 {
    FrameParams::default().sun_dir
}

/// Pixel of a point relative to `TestScene::ORIGIN` (None: behind the camera or off screen).
fn project(cam: &Camera, p: glam::Vec3) -> Option<(u32, u32)> {
    let rel = (TestScene::ORIGIN + p.as_dvec3() - cam.position).as_vec3();
    let c = cam.relative_view_proj(W as f32 / H as f32) * rel.extend(1.0);
    if c.w <= 0.0 {
        return None;
    }
    let (x, y) = ((c.x / c.w * 0.5 + 0.5) * W as f32, (0.5 - c.y / c.w * 0.5) * H as f32);
    (x >= 2.0 && y >= 2.0 && x < (W - 2) as f32 && y < (H - 2) as f32).then_some((x as u32, y as u32))
}

fn lum(p: [u8; 4]) -> f32 {
    0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32
}

/// Mean luminance of the 3x3 block around (x, y).
fn lum3(px: &[[u8; 4]], (x, y): (u32, u32)) -> f32 {
    let mut s = 0.0;
    for dy in 0..3 {
        for dx in 0..3 {
            s += lum(at(px, x + dx - 1, y + dy - 1));
        }
    }
    s / 9.0
}

/// The cascade resolve picks for camera-relative `p` (lighting.slang `shadow_cascade`).
fn cascade_of(plan: &light::CascadePlan, p: glam::Vec3) -> u32 {
    let margin = 1.0 - light::EDGE_MARGIN_TEXELS * 2.0 / plan.tile as f32;
    (0..plan.count).find(|&c| {
        let s = plan.matrices[c as usize] * p.extend(1.0);
        (0.0..=1.0).contains(&s.z) && s.x.abs().max(s.y.abs()) <= margin
    }).unwrap_or(4)
}

/// A slab floating at `d / 4` over the ground casts a square shadow; viewed from distance `d`
/// on the side away from the sun, the shadow's centre must be much darker than lit ground next
/// to it. `d` is chosen so the shadow falls in each of the four cascades in turn.
#[test]
fn shadow_regions_are_dark_in_every_cascade() {
    let Some(mut rig) = Rig::bare() else { return };
    let l = sun();
    let dists = [12.0f32, 90.0, 200.0, 380.0];
    let mut boxes = vec![GROUND];
    let mut cases = Vec::new();
    for (i, &d) in dists.iter().enumerate() {
        // Spread the slabs sideways so no slab shades another's case.
        let shadow = glam::Vec3::new(i as f32 * 900.0 - 1350.0, 0.0, 0.0);
        let (s, h) = (d / 6.0, d / 4.0);
        let slab = shadow + glam::Vec3::new(l.x, 0.0, l.z) / l.y * h + glam::Vec3::Y * h;
        boxes.push((slab.to_array(), [s, s / 20.0, s], [0.7, 0.7, 0.7]));
        let eye = shadow + glam::Vec3::new(-0.3, 0.6, -0.75).normalize() * d;
        let cam = Camera::look_at(TestScene::ORIGIN + eye.as_dvec3(), TestScene::ORIGIN + shadow.as_dvec3());
        let lit = shadow + glam::Vec3::new(-l.z, 0.0, l.x).normalize() * s * 0.9;
        cases.push((cam, shadow, lit));
    }
    load_boxes(&mut rig, &boxes);
    let settings = *rig.r().lighting();
    let (mut seen, mut bad) = (Vec::new(), Vec::new());
    for (i, (cam, shadow, lit)) in cases.iter().enumerate() {
        let plan = light::plan_cascades(&settings, cam, W as f32 / H as f32, sun());
        let c = cascade_of(&plan, (TestScene::ORIGIN + shadow.as_dvec3() - cam.position).as_vec3());
        // Two frames: the first fills the HZB history.
        rig.frame(*cam, SceneMode::Scene, false);
        let px = rig.image(*cam);
        dump(&px, &format!("m4d_shadow_{i}"));
        let (ps, pl) = (project(cam, *shadow).expect("shadow on screen"), project(cam, *lit).expect("lit point on screen"));
        let (ls, ll) = (lum3(&px, ps), lum3(&px, pl));
        println!("cascade {c}: shadow {ls:.1} at {ps:?}, lit {ll:.1} at {pl:?}");
        if ls >= 0.6 * ll {
            bad.push(format!("case {i} (cascade {c}): shadow {ls} not darker than lit {ll}"));
        }
        seen.push(c);
    }
    assert!(bad.is_empty(), "{bad:?}");
    assert_eq!(seen, vec![0, 1, 2, 3], "the cases should cover every cascade");
    rig.destroy();
}

fn shadow_mask(px: &[[u8; 4]]) -> Vec<bool> {
    // Shadowed ground is lit by the blue sky only; lit ground and boxes are not blue-dominant.
    px.iter().map(|p| p[2] as i32 > p[0] as i32 + 40 && (p[2] as i32) < 200).collect()
}

/// Pixels of `a` whose mask value appears nowhere within 1 px in `b` (an edge moved > 1 px).
fn moved_more_than_1px(a: &[bool], b: &[bool]) -> usize {
    let mut n = 0;
    for y in 1..H - 1 {
        for x in 1..W - 1 {
            let v = a[(y * W + x) as usize];
            let near = (0..3).any(|dy| (0..3).any(|dx| b[((y + dy - 1) * W + x + dx - 1) as usize] == v));
            n += (!near) as usize;
        }
    }
    n
}

/// Sub-texel camera moves (a quarter of a cascade-0 texel each) must not make shadow edges
/// swim: texel snapping keeps every edge within 1 px between consecutive frames.
#[test]
fn cascade_edges_stable_under_subtexel_camera_moves() {
    let Some(mut rig) = Rig::new() else { return };
    let base = TestScene::test_camera();
    let settings = *rig.r().lighting();
    let texel = light::plan_cascades(&settings, &base, W as f32 / H as f32, sun()).cascade[0][0] as f64;
    let right = base.right().as_dvec3();
    let mut prev: Option<Vec<bool>> = None;
    let mut worst = 0;
    let mut shadowed = Vec::new();
    for k in 0..12 {
        let mut cam = base;
        cam.position += right * (texel * 0.25 * k as f64);
        rig.frame(cam, SceneMode::Scene, false);
        let px = rig.image(cam);
        let m = shadow_mask(&px);
        shadowed.push(m.iter().filter(|&&v| v).count());
        if let Some(p) = &prev {
            worst = worst.max(moved_more_than_1px(p, &m)).max(moved_more_than_1px(&m, p));
        }
        prev = Some(m);
    }
    println!("cascade 0 texel {texel:.4} m; shadowed pixels per frame {shadowed:?}; pixels whose edge moved > 1 px: {worst}");
    assert!(shadowed[0] > 200, "the test scene should show shadows ({} px)", shadowed[0]);
    assert_eq!(worst, 0, "shadow edges moved more than 1 px under sub-texel camera moves");
    rig.destroy();
}

/// GTAO darkens the ground in the corner where a box meets it and leaves open ground (no
/// occluder within the radius) unchanged.
#[test]
fn ao_darkens_a_corner_and_leaves_a_plane_alone() {
    let Some(mut rig) = Rig::bare() else { return };
    load_boxes(&mut rig, &[GROUND, ([0.0, 1.0, 0.0], [2.0, 2.0, 2.0], [0.7, 0.7, 0.7])]);
    // Look at the box's sun-facing side so the corner is lit and unshadowed either way.
    let l = sun();
    let eye = glam::Vec3::new(l.x, 0.0, l.z).normalize() * 7.0 + glam::Vec3::Y * 3.0;
    let cam = Camera::look_at(TestScene::ORIGIN + eye.as_dvec3(), TestScene::ORIGIN + glam::DVec3::new(0.0, 0.5, 0.0));
    let shot = |rig: &mut Rig, ao: bool| {
        let s = LightingSettings { ao, shadows: false, ..*rig.r().lighting() };
        rig.r().set_lighting(s);
        rig.frame(cam, SceneMode::Scene, false);
        rig.image(cam)
    };
    let off = shot(&mut rig, false);
    let on = shot(&mut rig, true);
    dump(&off, "m4d_ao_off");
    dump(&on, "m4d_ao_on");
    // Ground just in front of the box's sun-facing face, and open ground 3 m further out.
    let f = glam::Vec3::new(l.x, 0.0, l.z).normalize();
    let corner = project(&cam, f * 1.1).expect("corner on screen");
    let open = project(&cam, f * 1.1 + glam::Vec3::new(-f.z, 0.0, f.x) * 3.5).expect("open ground on screen");
    let (c_off, c_on) = (lum3(&off, corner), lum3(&on, corner));
    let (o_off, o_on) = (lum3(&off, open), lum3(&on, open));
    println!("corner {corner:?}: {c_off:.1} -> {c_on:.1}; open {open:?}: {o_off:.1} -> {o_on:.1}");
    assert!(c_on < c_off - 4.0, "AO should darken the corner ({c_off} -> {c_on})");
    assert!((o_on - o_off).abs() <= 2.0, "AO changed open ground ({o_off} -> {o_on})");
    rig.destroy();
}
